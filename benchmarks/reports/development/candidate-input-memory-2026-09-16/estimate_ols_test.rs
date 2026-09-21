pub fn estimate_ols_test(args: &[String], spi: &mut dyn SpiApi) -> Result<()> {
    #[cfg(test)]
    let _action_observation_guard = instrumentation::lock_action_observation();
    let started = Instant::now();
    let resources_start = instrumentation::begin_observation();
    let local_bundle = read_local_estimate_bundle(args, spi)?.map(|value| [value]);
    let mut specification = EstimateActionSpec::parse(
        local_bundle.as_ref().map_or(args, |values| values.as_slice()),
    )?;
    // The expanded local bundle dies here; only the original ABI argv
    // remains live through numerical work and publication.
    specification.transport_parent_bytes = crate::metadata_memory::argument_parent_bytes(args)?;
    drop(local_bundle);
    let mut action = specification.decode_and_validate(spi)?;
    drop(specification);
    let preflight_seconds = started.elapsed().as_secs_f64();
    let mut input_memory = loading_memory(
        &action.request, spi.nobs()?, 0,
        action.metadata_owner_bytes, action.memory_plan.usable_bytes,
    )?;
    let loaded = load_ols_input_with_memory(
        &action.request,
        action.candidate_all,
        action.input_complete,
        &mut input_memory,
        spi,
    )?;
    action.input_peak_bytes = input_memory.peak();
    let input_load_seconds = started.elapsed().as_secs_f64() - preflight_seconds;
    let observation = ActionObservation {
        started,
        resources_start,
        preflight_seconds,
        input_load_seconds,
    };
    let categorical = action.request.absorbed_terms.iter().any(|term| {
        term.categorical_sources.len() > 1
            || !term.slope_sources.is_empty()
            || !term.saved_name.is_empty()
    });
    match (action.vce, categorical) {
        (OlsVceKind::Iid, false) => {
            let trace_enabled = std::env::var_os("FEREG_PHASE_TIMING").is_some();
            let trace_started = Instant::now();
            let prepared = prepare_loaded_ols(loaded, action.fit.execution.threads, |loaded| {
                prepare_ols_data_owned(&action.request, loaded)
            })?;
            action.admit_prepared_parent(prepared.owned_buffer_bytes())?;
            let prepare_seconds = trace_started.elapsed().as_secs_f64();
            let inference_started = Instant::now();
            let inference = fit_prepared_ols_unadjusted_map(
                &prepared,
                PreparedOlsUnadjustedConfig {
                    fit: action.fit,
                    absorbed_dof: action.absorbed_dof,
                    ..PreparedOlsUnadjustedConfig::default()
                },
            )?;
            action.admit_output_parent(inference.owned_buffer_bytes())?;
            let inference_seconds = inference_started.elapsed().as_secs_f64();
            let outputs_started = Instant::now();
            let result_data = PreparedOlsResultData::legacy(&prepared);
            let derived = build_derived_observation_outputs(
                result_data,
                None,
                inference.fit(),
                &action.outputs,
                false,
                action.fit,
            )?;
            let outputs_seconds = outputs_started.elapsed().as_secs_f64();
            let shadow_started = Instant::now();
            let shadow = build_shadow_result(
                result_data,
                None,
                CertifiedOlsInference::Iid(&inference),
                action.outputs.residual.is_some(),
            )?;
            let shadow_seconds = shadow_started.elapsed().as_secs_f64();
            if trace_enabled {
                eprintln!(
                    "FEREG_PHASE|iid_plugin|prepare={prepare_seconds:.9}|inference={inference_seconds:.9}|outputs={outputs_seconds:.9}|shadow={shadow_seconds:.9}|prestore_total={:.9}",
                    trace_started.elapsed().as_secs_f64()
                );
            }
            precommit_and_store(&action, &shadow, &derived, &observation, spi)
        }
        (OlsVceKind::Robust, false) => {
            let trace_enabled = std::env::var_os("FEREG_PHASE_TIMING").is_some();
            let trace_started = Instant::now();
            let prepared = prepare_loaded_ols(loaded, action.fit.execution.threads, |loaded| {
                prepare_ols_data_owned(&action.request, loaded)
            })?;
            action.admit_prepared_parent(prepared.owned_buffer_bytes())?;
            let prepare_seconds = trace_started.elapsed().as_secs_f64();
            let inference_started = Instant::now();
            let inference = fit_prepared_ols_robust_map(
                &prepared,
                PreparedOlsRobustConfig {
                    fit: action.fit,
                    absorbed_dof: action.absorbed_dof,
                    ..PreparedOlsRobustConfig::default()
                },
            )?;
            action.admit_output_parent(inference.owned_buffer_bytes())?;
            let inference_seconds = inference_started.elapsed().as_secs_f64();
            let outputs_started = Instant::now();
            let result_data = PreparedOlsResultData::legacy(&prepared);
            let derived = build_derived_observation_outputs(
                result_data,
                None,
                inference.fit(),
                &action.outputs,
                false,
                action.fit,
            )?;
            let outputs_seconds = outputs_started.elapsed().as_secs_f64();
            let shadow_started = Instant::now();
            let shadow = build_shadow_result(
                result_data,
                None,
                CertifiedOlsInference::Robust(&inference),
                action.outputs.residual.is_some(),
            )?;
            let shadow_seconds = shadow_started.elapsed().as_secs_f64();
            if trace_enabled {
                eprintln!(
                    "FEREG_PHASE|robust_plugin|prepare={prepare_seconds:.9}|inference={inference_seconds:.9}|outputs={outputs_seconds:.9}|shadow={shadow_seconds:.9}|prestore_total={:.9}",
                    trace_started.elapsed().as_secs_f64()
                );
            }
            precommit_and_store(&action, &shadow, &derived, &observation, spi)
        }
        (OlsVceKind::Clustered, false) => {
            let request = OlsClusteredRequestV1 {
                ols: action.request.clone(),
                clusters: action.clusters.clone(),
            };
            let prepared = prepare_loaded_ols(loaded, action.fit.execution.threads, |loaded| {
                prepare_ols_clustered_data_owned(&request, loaded)
            })?;
            drop(request);
            action.admit_prepared_parent(prepared.ols().owned_buffer_bytes() + prepared.clusters().owned_buffer_bytes())?;
            let inference = fit_prepared_ols_clustered_map(
                &prepared,
                PreparedOlsClusteredConfig {
                    fit: action.fit,
                    absorbed_dof: action.absorbed_dof,
                    account_cluster_nesting: action.account_cluster_nesting,
                    ..PreparedOlsClusteredConfig::default()
                },
            )?;
            action.admit_output_parent(inference.owned_buffer_bytes())?;
            let result_data = PreparedOlsResultData::legacy(prepared.ols());
            let derived = build_derived_observation_outputs(
                result_data,
                None,
                inference.fit(),
                &action.outputs,
                false,
                action.fit,
            )?;
            let shadow = build_shadow_result(
                result_data,
                Some(PreparedOlsClusteredResultData::legacy(&prepared)),
                CertifiedOlsInference::Clustered(&inference),
                action.outputs.residual.is_some(),
            )?;
            precommit_and_store(&action, &shadow, &derived, &observation, spi)
        }
        (OlsVceKind::Iid, true) => {
            let prepared = prepare_loaded_ols(loaded, action.fit.execution.threads, |loaded| {
                prepare_categorical_ols_data_owned(&action.request, loaded)
            })?;
            action.admit_prepared_parent(prepared.owned_buffer_bytes())?;
            let inference = fit_prepared_categorical_ols_unadjusted_map(
                &prepared,
                PreparedOlsUnadjustedConfig {
                    fit: action.fit,
                    absorbed_dof: action.absorbed_dof,
                    ..PreparedOlsUnadjustedConfig::default()
                },
            )?;
            action.admit_output_parent(inference.owned_buffer_bytes())?;
            let result_data = PreparedOlsResultData::categorical(&prepared);
            let derived = build_derived_observation_outputs(
                result_data,
                Some(&prepared),
                inference.fit(),
                &action.outputs,
                normalization_known_connected(inference.absorbed_dof()),
                action.fit,
            )?;
            let shadow = build_shadow_result(
                result_data,
                None,
                CertifiedOlsInference::CategoricalIid(&inference),
                action.outputs.residual.is_some(),
            )?;
            precommit_and_store(&action, &shadow, &derived, &observation, spi)
        }
        (OlsVceKind::Robust, true) => {
            let prepared = prepare_loaded_ols(loaded, action.fit.execution.threads, |loaded| {
                prepare_categorical_ols_data_owned(&action.request, loaded)
            })?;
            action.admit_prepared_parent(prepared.owned_buffer_bytes())?;
            let inference = fit_prepared_categorical_ols_robust_map(
                &prepared,
                PreparedOlsRobustConfig {
                    fit: action.fit,
                    absorbed_dof: action.absorbed_dof,
                    ..PreparedOlsRobustConfig::default()
                },
            )?;
            action.admit_output_parent(inference.owned_buffer_bytes())?;
            let result_data = PreparedOlsResultData::categorical(&prepared);
            let derived = build_derived_observation_outputs(
                result_data,
                Some(&prepared),
                inference.fit(),
                &action.outputs,
                normalization_known_connected(inference.absorbed_dof()),
                action.fit,
            )?;
            let shadow = build_shadow_result(
                result_data,
                None,
                CertifiedOlsInference::CategoricalRobust(&inference),
                action.outputs.residual.is_some(),
            )?;
            precommit_and_store(&action, &shadow, &derived, &observation, spi)
        }
        (OlsVceKind::Clustered, true) => {
            let request = OlsClusteredRequestV1 {
                ols: action.request.clone(),
                clusters: action.clusters.clone(),
            };
            let prepared = prepare_loaded_ols(loaded, action.fit.execution.threads, |loaded| {
                prepare_categorical_ols_clustered_data_owned(&request, loaded)
            })?;
            drop(request);
            action.admit_prepared_parent(prepared.ols().owned_buffer_bytes() + prepared.clusters().owned_buffer_bytes())?;
            let inference = fit_prepared_categorical_ols_clustered_map(
                &prepared,
                PreparedOlsClusteredConfig {
                    fit: action.fit,
                    absorbed_dof: action.absorbed_dof,
                    account_cluster_nesting: action.account_cluster_nesting,
                    ..PreparedOlsClusteredConfig::default()
                },
            )?;
            action.admit_output_parent(inference.owned_buffer_bytes())?;
            let result_data = PreparedOlsResultData::categorical(prepared.ols());
            let derived = build_derived_observation_outputs(
                result_data,
                Some(prepared.ols()),
                inference.fit(),
                &action.outputs,
                normalization_known_connected(inference.absorbed_dof()),
                action.fit,
            )?;
            let shadow = build_shadow_result(
                result_data,
                Some(PreparedOlsClusteredResultData::categorical(&prepared)),
                CertifiedOlsInference::CategoricalClustered(&inference),
                action.outputs.residual.is_some(),
            )?;
            precommit_and_store(&action, &shadow, &derived, &observation, spi)
        }
    }
}
