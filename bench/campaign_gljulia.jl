# Pinned GLFixedEffectModels.jl PPML arm: one warmed, timed Poisson fit per process.
#
# Mirrors campaign_julia.jl: the same inputs, memory handshake, timing window
# and output files. GLFixedEffectModels 0.5.6 requires FixedEffectModels 1.x,
# so it runs in its own environment (julia/ppml) and depot rather than the
# OLS arm's FixedEffectModels 2.1.0 project. `glfixedeffects` keeps the
# package's stopping defaults (center_tol and dev_tol 1e-8); the strict arm
# uses the policy's center_tol/dev_tol. Separation detection keeps the package
# default (none), so separated rows and controls are retained. The warm-up
# fit repeats the timed specification on the small warm input.
#
# Reported conventions, applied outside the timed window:
# - covariance.csv rescales the package covariance to the campaign's PPML
#   convention (robust N/(N-1), clustered G/(G-1)); covariance-stock.csv keeps
#   the package value. The package applies no factor to robust covariance and
#   (N-1)/dof_residual * G/(G-1) to clustered covariance (Vcov.S_hat).
# - ll is the Poisson log likelihood with the log-gamma normalization,
#   -deviance/2 + sum(y log y - y - loggamma(y+1)), which is defined for
#   fractional outcomes; ll_package is the package's value, which uses the
#   integer Poisson density and is -Inf for fractional outcomes.
# - df_a counts every retained FE level, as the package does; campaign.py maps
#   it to the common FE-span rank.
using GLFixedEffectModels, GLM, Distributions, DataFrames, ReadStatTables, SHA, JSON
using LinearAlgebra

configuration(path) = Dict(String(a) => String(b) for (a, b) in (split(line, '\t'; limit=2) for line in eachline(path)))

# Indirect dependencies, loaded by GLFixedEffectModels, without widening the pinned project.
loaded(name) = only(m for (id, m) in Base.loaded_modules if id.name == name)

function wait_for(path, seconds=120.0)
    started = time()
    while !isfile(path)
        time() - started > seconds && error("campaign handshake timed out: " * basename(path))
        sleep(0.001)
    end
end

function column_hash(values)
    ENDIAN_BOM == 0x04030201 || error("campaign column hashes require little-endian doubles")
    bytes2hex(sha256(reinterpret(UInt8, Vector{Float64}(values))))
end

function model_formula(effects, k)
    terms = join(["x$i" for i in 1:k], " + ")
    absorb = join(["fe($e)" for e in effects], " + ")
    eval(Meta.parse("@formula(y ~ $terms + $absorb)"))
end

covariance(vce) = vce == "robust" ? Vcov.robust() : vce == "cluster" ? Vcov.cluster(:cluster) :
    error("PPML covariance must be robust or cluster")

function load(path, q)
    frame = DataFrame(readstat(path))
    for name in ["g$j" for j in 1:q]
        frame[!, name] = Vector{Int64}(frame[!, name])
    end
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

function write_coefficients(output, suffix, labels, b, V)
    open(joinpath(output, "coefficients$suffix.csv"), "w") do io
        println(io, "name,value")
        foreach(i -> println(io, labels[i], ',', repr(b[i])), eachindex(b))
    end
    open(joinpath(output, "covariance$suffix.csv"), "w") do io
        println(io, "row,column,value")
        for i in eachindex(b), j in eachindex(b)
            println(io, labels[i], ',', labels[j], ',', repr(V[i, j]))
        end
    end
end

"Poisson log likelihood with the log-gamma normalization, from the deviance."
function poisson_loglikelihood(deviance, y)
    loggamma = loaded("SpecialFunctions").loggamma
    -deviance / 2 + sum((v > 0 ? v * log(v) : 0.0) - v - loggamma(v + 1) for v in y)
end

function main(path)
    c = configuration(path)
    c["model"] == "ppml" || error("GLFixedEffectModels.jl runs PPML only")
    k, q, threads = parse(Int, c["k"]), parse(Int, c["effects"]), parse(Int, c["threads"])
    q > 0 || error("GLFixedEffectModels.jl requires at least one absorbed effect")
    vce, arm, save = c["vce"], c["arm"], c["save_fe"] == "1"
    version = string(pkgversion(GLFixedEffectModels))
    version == c["glfixedeffectmodels"] || error("GLFixedEffectModels $version is not the pinned $(c["glfixedeffectmodels"])")
    fem = string(pkgversion(loaded("FixedEffectModels")))
    fem == c["fixedeffectmodels"] || error("FixedEffectModels $fem is not the pinned $(c["fixedeffectmodels"])")
    string(VERSION) == c["julia_version"] || error("Julia $VERSION is not the pinned $(c["julia_version"])")
    Threads.nthreads() == threads || error("Julia started $(Threads.nthreads()) threads, not $threads")
    BLAS.set_num_threads(1)
    output = c["output"]
    options = (save = save ? [:fe] : Symbol[], nthreads = threads)
    if endswith(arm, "-strict")
        options = merge(options, (center_tol = parse(Float64, c["strict_center_tol"]),
                                  dev_tol = parse(Float64, c["strict_dev_tol"])))
    end
    effects = ["g$j" for j in 1:q]
    fit_model(data) = nlreg(data, model_formula(effects, k), Poisson(), LogLink(), covariance(vce); options...)

    warm = load(c["warm"], 1)
    for j in 2:q
        warm[!, "g$j"] = mod.(warm.g1, j == 2 ? 7 : 5) .+ 1
    end
    warm_seconds = @elapsed begin
        w = fit_model(warm)
        w.converged || error("GLFixedEffectModels warm-up did not converge")
        save && fe(w)
    end
    warm = nothing

    data = load(c["dataset"], q)
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
        fit = fit_model(data)
        fit.converged || error("GLFixedEffectModels did not converge")
        save && (saved = fe(fit))
    catch failure
        error_message, fit = sprint(showerror, failure), nothing
    end
    seconds = (time_ns() - started) / 1e9
    touch(joinpath(output, "DONE"))
    wait_for(joinpath(output, "ACK"))

    meta = Pair{String, Any}["rc" => fit === nothing ? 1 : 0, "seconds" => seconds, "complete" => 1,
                             "julia_version" => string(VERSION), "glfixedeffectmodels_version" => version,
                             "fixedeffectmodels_version" => fem,
                             "requested_threads" => threads, "configured_threads" => Threads.nthreads(),
                             "reference_profile" => arm, "tolerance" => get(options, :center_tol, "default"),
                             "deviance_tolerance" => get(options, :dev_tol, "default"),
                             "separation_policy" => "package default (none)", "warm_seconds" => warm_seconds]
    if fit !== nothing
        b, Vstock = coef(fit), Matrix(vcov(fit))
        retained = fit.esample
        n = nobs(fit)
        clusters = vce == "cluster" ? length(unique(view(data.cluster, retained))) : nothing
        # Undo the package's small-sample factor and apply the campaign's (see above).
        multiplier = vce == "robust" ? n / (n - 1) : dof_residual(fit) / (n - 1)
        # Collinear or separated slopes have NaN variances; Stata names them o.x.
        omitted = .!isfinite.(diag(Vstock))
        labels = String.(coefnames(fit))
        normalized = [omitted[i] ? "o." * labels[i] : labels[i] for i in eachindex(labels)]
        write_coefficients(output, "", normalized, b, Vstock .* multiplier)
        write_coefficients(output, "-stock", normalized, b, Vstock)
        rows = DataFrame(obs_id = data.obs_id, retained = Int8.(retained))
        if saved !== nothing
            X = Matrix{Float64}(data[!, ["x$i" for i in 1:k]])
            slopes = [findfirst(==("x$i"), labels) for i in 1:k]
            beta = [omitted[s] ? 0.0 : b[s] for s in slopes]
            predictor = X * beta
            fitted = Vector{Union{Float64, Missing}}(missing, nrow(data))
            channels = [saved[!, j] for j in 1:q]
            fitted[retained] .= exp.(predictor[retained] .+ sum(Vector{Float64}(channel[retained]) for channel in channels))
            rows.fitted = fitted
            for j in 1:q
                rows[!, "saved$j"] = channels[j]
            end
        end
        write_rows(joinpath(output, "rows.csv"), rows)
        levels = Int[length(unique(view(data[!, e], retained))) for e in effects]
        slope_names = Set("x$i" for i in 1:k)
        slope_rows = [i for i in eachindex(labels) if labels[i] in slope_names]
        append!(meta, ["N" => n, "df_a" => sum(levels), "df_m" => count(!, omitted),
                       "df_r" => dof_residual(fit), "converged" => 1, "iterations" => fit.iterations,
                       "finite_slope_rank" => count(i -> !omitted[i], slope_rows),
                       "slope_omissions" => count(i -> omitted[i], slope_rows),
                       "covariance_policy" => "ppml-$vce-K.adj=FALSE", "covariance_multiplier" => repr(multiplier),
                       "ll" => repr(poisson_loglikelihood(deviance(fit), view(data.y, retained))),
                       "ll_package" => repr(loglikelihood(fit)), "deviance" => repr(deviance(fit))])
        vce == "cluster" && push!(meta, "N_clust" => clusters)
    end
    error_message === nothing || push!(meta, "error" => error_message)
    write_metadata(joinpath(output, "metadata.tsv"), meta)
    println("CAMPAIGN_COMPLETE rc=", fit === nothing ? 1 : 0)
    fit === nothing ? 1 : 0
end

exit(main(ARGS[1]))
