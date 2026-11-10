# GLFixedEffectModels.jl PPML, in its own pinned Julia environment.
using GLFixedEffectModels, GLM, Distributions, DataFrames, ReadStatTables, SHA, JSON
using LinearAlgebra

configuration(path) = Dict(String(a) => String(b) for (a,b) in (split(line, '\t'; limit=2) for line in eachline(path)))
function wait_for(path)
    started = time()
    while !isfile(path)
        time()-started > 120 && error("campaign handshake timed out: " * basename(path))
        sleep(0.001)
    end
end
function load_data(path, q)
    data = DataFrame(readstat(path))
    for name in ["g$j" for j in 1:q]
        data[!,name] = Int64.(data[!,name])
    end
    "cluster" in names(data) && (data.cluster = Int64.(data.cluster))
    data
end
function formula(q,k)
    rhs = join(["x$j" for j in 1:k], " + ")
    q > 0 && (rhs *= " + " * join(["fe(g$j)" for j in 1:q], " + "))
    eval(Meta.parse("@formula(y ~ 1 + $rhs)"))
end
function nested(effect, cluster)
    owner = Dict{Int64,Int64}()
    all(get!(owner, a, b) == b for (a,b) in zip(effect,cluster))
end
function write_rows(path, rows)
    open(path,"w") do io
        println(io,join(names(rows),','))
        for row in eachrow(rows)
            println(io,join([ismissing(x) ? "." : repr(x) for x in row],','))
        end
    end
end
function write_metadata(path, meta)
    open(path,"w") do io
        for (key,value) in meta
            println(io,key,'\t',replace(string(value),'\t'=>' ','\n'=>' '))
        end
    end
end
function main(path)
    c = configuration(path)
    c["model"] == "ppml" || error("GLFixedEffectModels arm requires PPML")
    string(VERSION) == c["julia_version"] || error("Julia version differs from pin")
    string(pkgversion(GLFixedEffectModels)) == c["glfixedeffectmodels"] || error("GLFixedEffectModels version differs from pin")
    k,q,threads = parse.(Int,(c["k"],c["effects"],c["threads"]))
    Threads.nthreads() == threads || error("incorrect Julia worker count")
    BLAS.set_num_threads(1)
    saved = c["save_fe"] == "1"
    strict = endswith(c["arm"],"-strict")
    vce = c["vce"]
    covariance = vce == "robust" ? Vcov.robust() : vce == "cluster" ? Vcov.cluster(:cluster) : error("PPML covariance must be robust or cluster")
    # Preserve the package's separation and small-sample defaults in both profiles.
    options = (save = saved ? [:mu,:eta,:fe] : Symbol[], verbose=false, nthreads=threads)
    strict && (options=merge(options,(center_tol=1e-12,dev_tol=1e-12,step_tol=1e-12)))
    fml = formula(q,k)
    warm = load_data(c["warm"],1)
    for j in 2:q
        warm[!,"g$j"] = mod.(warm.g1,j == 2 ? 7 : 5) .+ 1
    end
    warm_seconds = @elapsed begin
        w = nlreg(warm,fml,Poisson(),LogLink(),covariance;options...)
        w.converged || error("warm-up failed to converge")
        saved && q > 0 && fe(w)
    end
    warm = nothing
    data = load_data(c["dataset"],q)
    identity = JSON.parsefile(replace(c["dataset"],r"\.dta$"=>".json"))
    ENDIAN_BOM == 0x04030201 || error("input hashes require little endian")
    for (name,expected) in identity["column_sha256"]
        bytes2hex(sha256(reinterpret(UInt8,Vector{Float64}(data[!,name])))) == expected || error("decoded input mismatch: $name")
    end
    GC.gc()
    output = c["output"]
    touch(joinpath(output,"READY"));wait_for(joinpath(output,"GO"))
    fit, failure = nothing,nothing
    started=time_ns()
    try
        fit = nlreg(data,fml,Poisson(),LogLink(),covariance;options...)
    catch e
        failure=sprint(showerror,e)
    end
    seconds=(time_ns()-started)/1e9
    touch(joinpath(output,"DONE"));wait_for(joinpath(output,"ACK"))
    meta=Pair{String,Any}["rc"=>(fit===nothing ? 1 : 0),"seconds"=>seconds,"complete"=>1,
        "warm_seconds"=>warm_seconds,"julia_version"=>string(VERSION),
        "glfixedeffectmodels_version"=>string(pkgversion(GLFixedEffectModels)),
        "requested_threads"=>threads,"configured_threads"=>Threads.nthreads(),"reference_profile"=>c["arm"],
        "center_tolerance"=>get(options,:center_tol,"default"),"deviance_tolerance"=>get(options,:dev_tol,"default"),
        "step_tolerance"=>get(options,:step_tol,"default"),"separation_policy"=>"package-default",
        "inference_policy"=>"package-default"]
    if fit !== nothing
        b,V=coef(fit),vcov(fit)
        labels=String.(coefnames(fit))
        # Preserve the package's NaN omission markers in raw files, and expose
        # Stata-style omitted names in the common finite-coefficient interface.
        omitted=.!isfinite.(diag(V))
        normalized=[omitted[i] ? "o."*labels[i] : labels[i] for i in eachindex(labels)]
        for (suffix,names) in [("",normalized),("-raw",labels)]
            open(joinpath(output,"coefficients$suffix.csv"),"w") do io
                println(io,"name,value")
                for i in eachindex(b);println(io,names[i],',',repr(b[i]));end
            end
            open(joinpath(output,"covariance$suffix.csv"),"w") do io
                println(io,"row,column,value")
                for i in eachindex(b),j in eachindex(b);println(io,names[i],',',names[j],',',repr(V[i,j]));end
            end
        end
        retained=fit.esample
        rows=DataFrame(obs_id=data.obs_id,retained=Int8.(retained))
        if saved
            rows.mu=fit.augmentdf.mu
            rows.eta=fit.augmentdf.eta
            # Recover the prediction from the requested FE output independently.
            predictor=zeros(nrow(data))
            for (name,value,omit) in zip(labels,b,omitted)
                omit && continue
                predictor .+= name == "(Intercept)" ? value : data[!,name].*value
            end
            if q > 0
                channels=fe(fit)
                for j in 1:q
                    rows[!,"saved$j"]=channels[!,j]
                    predictor[retained] .+= channels[retained,j]
                end
            end
            rows.fitted=Vector{Union{Missing,Float64}}(missing,nrow(data))
            rows[retained,:fitted]=exp.(predictor[retained])
        end
        write_rows(joinpath(output,"rows.csv"),rows)
        levels=[length(unique(data[retained,"g$j"])) for j in 1:q]
        absorbed=sum(levels;init=0)
        if vce=="cluster"
            groups=data.cluster[retained]
            absorbed=sum((nested(data[retained,"g$j"],groups) ? 1 : levels[j] for j in 1:q);init=0)
            push!(meta,"N_clust"=>length(unique(groups)))
        end
        slopes=[i for i in eachindex(labels) if labels[i] in ["x$j" for j in 1:k]]
        append!(meta,["N"=>nobs(fit),"df_a"=>absorbed,"df_a_unadjusted"=>sum(levels;init=0),
            "df_m"=>count(!,omitted),"df_r"=>dof_residual(fit),"correction_df"=>dof_residual(fit),
            "finite_slope_rank"=>count(i->!omitted[i],slopes),"slope_omissions"=>count(i->omitted[i],slopes),
            "converged"=>Int(fit.converged),"iterations"=>fit.iterations,"ll"=>repr(loglikelihood(fit)),
            "deviance"=>repr(deviance(fit)),"covariance_policy"=>"ppml-$vce-stock"])
    end
    failure===nothing || push!(meta,"error"=>failure)
    write_metadata(joinpath(output,"metadata.tsv"),meta)
    println("CAMPAIGN_COMPLETE rc=",fit===nothing ? 1 : 0)
    fit===nothing ? 1 : 0
end
exit(main(ARGS[1]))
