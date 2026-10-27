# Pinned FixedEffectModels.jl campaign arm: one warmed, timed OLS fit per process.
#
# Mirrors campaign_r.R: the same inputs, memory handshake, timing window and
# output files. `julia` keeps the package's default tolerance (1e-6) and
# `julia-strict` uses 1e-10 for estimation and FE recovery. `julia --threads`
# sets the worker count, since its `nthreads` option is deprecated. The warm-up
# fit repeats the timed specification on the small warm input, so compilation
# stays outside the timed command.
using FixedEffectModels, DataFrames, ReadStatTables, SHA, JSON
using LinearAlgebra

configuration(path) = Dict(String(a) => String(b) for (a, b) in (split(line, '\t'; limit=2) for line in eachline(path)))

function wait_for(path, seconds=120.0)
    started = time()
    while !isfile(path)
        time() - started > seconds && error("campaign handshake timed out: " * basename(path))
        sleep(0.001)
    end
end

function column_hash(values)
    bytes = reinterpret(UInt8, Vector{Float64}(values))
    ENDIAN_BOM == 0x04030201 || error("campaign column hashes require little-endian doubles")
    bytes2hex(sha256(bytes))
end

function nested(effect, cluster)
    owner = Dict{eltype(effect), eltype(cluster)}()
    for (level, group) in zip(effect, cluster)
        get!(owner, level, group) == group || return false
    end
    true
end

function model_formula(effects, k)
    terms = join(["x$i" for i in 1:k], " + ")
    absorb = join(["fe($e)" for e in effects], " + ")
    eval(Meta.parse("@formula(y ~ $terms + $absorb)"))
end

covariance(vce) = vce == "iid" ? Vcov.simple() : vce == "robust" ? Vcov.robust() : Vcov.cluster(:cluster)

function load(path, k, q)
    frame = DataFrame(readstat(path))
    for name in ["g$j" for j in 1:q]
        frame[!, name] = Vector{Int64}(frame[!, name])
    end
    # Real panels carry no cluster variable; their covariance is regular.
    "cluster" in names(frame) && (frame.cluster = Vector{Int64}(frame.cluster))
    frame
end

write_value(io, value::AbstractFloat) = print(io, repr(value))
write_value(io, value::Integer) = print(io, value)
write_value(io, ::Missing) = print(io, '.')

function write_rows(path, rows)
    columns = [rows[!, name] for name in names(rows)]
    open(path, "w") do io
        println(io, join(names(rows), ','))
        for i in 1:nrow(rows)
            for (j, column) in enumerate(columns)
                j > 1 && print(io, ',')
                write_value(io, column[i])
            end
            print(io, '\n')
        end
    end
end

write_metadata(path, meta) = open(path, "w") do io
    for (key, value) in meta
        println(io, key, '\t', replace(string(value), '\t' => ' ', '\n' => ' '))
    end
end

function main(path)
    c = configuration(path)
    model, vce, arm = c["model"], c["vce"], c["arm"]
    model == "ols" || error("FixedEffectModels.jl runs linear models only")
    k, q, threads = parse(Int, c["k"]), parse(Int, c["effects"]), parse(Int, c["threads"])
    save = c["save_fe"] == "1"
    version = string(pkgversion(FixedEffectModels))
    version == c["fixedeffectmodels"] || error("FixedEffectModels $version is not the pinned $(c["fixedeffectmodels"])")
    string(VERSION) == c["julia_version"] || error("Julia $VERSION is not the pinned $(c["julia_version"])")
    Threads.nthreads() == threads || error("Julia started $(Threads.nthreads()) threads, not $threads")
    BLAS.set_num_threads(1)
    output = c["output"]
    options = (save = save ? :fe : :none, progress_bar = false)
    arm == "julia-strict" && (options = merge(options, (tol = 1e-10,)))

    warm = load(c["warm"], k, 1)
    for j in 2:q
        warm[!, "g$j"] = mod.(warm.g1, j == 2 ? 7 : 5) .+ 1
    end
    warm_seconds = @elapsed reg(warm, model_formula(["g$j" for j in 1:q], k), covariance(vce); options...)
    warm = nothing

    data = load(c["dataset"], k, q)
    identity = JSON.parsefile(replace(c["dataset"], r"\.dta$" => ".json"))
    for (name, expected) in identity["column_sha256"]
        column_hash(data[!, name]) == expected || error("decoded input mismatch: $name")
    end
    GC.gc()
    touch(joinpath(output, "READY"))
    wait_for(joinpath(output, "GO"))
    error_message, fit, saved = nothing, nothing, nothing
    started = time_ns()
    try
        fit = reg(data, model_formula(["g$j" for j in 1:q], k), covariance(vce); options...)
        fit.converged || error("FixedEffectModels did not converge")
        save && (saved = fe(fit))
    catch failure
        error_message, fit = sprint(showerror, failure), nothing
    end
    seconds = (time_ns() - started) / 1e9
    touch(joinpath(output, "DONE"))
    wait_for(joinpath(output, "ACK"))

    meta = Pair{String, Any}["rc" => fit === nothing ? 1 : 0, "seconds" => seconds, "complete" => 1,
                             "julia_version" => string(VERSION), "fixedeffectmodels_version" => version,
                             "requested_threads" => threads, "configured_threads" => Threads.nthreads(),
                             "reference_profile" => arm, "tolerance" => get(options, :tol, "default"),
                             "warm_seconds" => warm_seconds]
    if fit !== nothing
        labels = coefnames(fit)
        b, V = coef(fit), vcov(fit)
        open(joinpath(output, "coefficients.csv"), "w") do io
            println(io, "name,value")
            foreach(i -> println(io, labels[i], ',', repr(b[i])), eachindex(b))
        end
        open(joinpath(output, "covariance.csv"), "w") do io
            println(io, "row,column,value")
            for i in eachindex(b), j in eachindex(b)
                println(io, labels[i], ',', labels[j], ',', repr(V[i, j]))
            end
        end
        retained = fit.esample
        rows = DataFrame(obs_id = data.obs_id, retained = Int8.(retained))
        if saved !== nothing
            X = Matrix{Float64}(data[!, ["x$i" for i in 1:k]])
            predictor = X * b
            fitted = Vector{Union{Float64, Missing}}(missing, nrow(data))
            channels = [saved[!, j] for j in 1:q]
            fitted[retained] .= predictor[retained] .+ sum(channel[retained] for channel in channels)
            rows.fitted = fitted
            for j in 1:q
                rows[!, "saved$j"] = channels[j]
            end
        end
        write_rows(joinpath(output, "rows.csv"), rows)
        effects = [view(data[!, "g$j"], retained) for j in 1:q]
        levels = [length(unique(effect)) for effect in effects]
        # FixedEffectModels counts every level; an effect nested in the
        # cluster variable absorbs one degree of freedom (fit.jl).
        absorbed = sum(levels)
        if vce == "cluster"
            clusters = view(data.cluster, retained)
            absorbed = sum(nested(effects[j], clusters) ? 1 : levels[j] for j in 1:q)
        end
        n = nobs(fit)
        append!(meta, ["N" => n, "df_a" => absorbed, "df_a_unadjusted" => sum(levels), "df_m" => k,
                       "df_r" => dof_residual(fit), "correction_df" => n - k - absorbed,
                       "converged" => 1, "iterations" => fit.iterations, "finite_slope_rank" => k,
                       "slope_omissions" => count(i -> V[i, i] == 0, eachindex(b)),
                       "covariance_policy" => "ols-$vce-stock", "rss" => repr(deviance(fit))])
        vce == "cluster" && push!(meta, "N_clust" => length(unique(clusters)))
    end
    error_message === nothing || push!(meta, "error" => error_message)
    write_metadata(joinpath(output, "metadata.tsv"), meta)
    println("CAMPAIGN_COMPLETE rc=", fit === nothing ? 1 : 0)
    fit === nothing ? 1 : 0
end

exit(main(ARGS[1]))
