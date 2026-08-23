#!/usr/bin/env python3
"""Use the categorical request planner for hidden estimate pre-read validation."""

from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
ESTIMATE = ROOT / "crates/stfixest-plugin/src/estimate.rs"
CONTRACT = ROOT / "tests/python/test_categorical_hidden_transport_contract.py"


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one occurrence, found {count}")
    return text.replace(old, new, 1)


estimate = ESTIMATE.read_text(encoding="utf-8")
estimate = replace_once(
    estimate,
    "use stfixest_core::error::{ErrorCode, Result, StFixestError};\n",
    "use stfixest_core::error::{ErrorCode, Result, StFixestError};\n"
    "use stfixest_core::fe::plan_categorical_absorbed_request;\n",
    "categorical request-planner import",
)
estimate = replace_once(
    estimate,
    "        let clusters = read_clusters(self, spi)?;\n"
    "        validate_vce(self.vce, &clusters, self.account_cluster_nesting)?;\n"
    "        let prepared_model = if self.vce == OlsVceKind::Clustered {\n"
    "            let clustered_request = OlsClusteredRequestV1 {\n"
    "                ols: request.clone(),\n"
    "                clusters: clusters.clone(),\n"
    "            };\n"
    "            // This validates cluster-only sources without reading observations.\n"
    "            validate_cluster_request_shape(&clustered_request)?;\n"
    "            clustered_request.validate_and_compile()?\n"
    "        } else {\n"
    "            request.validate_and_compile()?\n"
    "        };\n",
    "        let clusters = read_clusters(self, spi)?;\n"
    "        validate_vce(self.vce, &clusters, self.account_cluster_nesting)?;\n"
    "        let categorical = request\n"
    "            .absorbed_terms\n"
    "            .iter()\n"
    "            .any(|term| term.categorical_sources.len() > 1);\n"
    "        let prepared_model = if categorical {\n"
    "            // Validate the original tuple identities and all-categorical\n"
    "            // intercept-only contract before any observation read. The\n"
    "            // availability request preserves the ordinary model and\n"
    "            // coefficient layout while replacing each tuple with one\n"
    "            // synthetic single-source availability marker, which is the\n"
    "            // only representation accepted by the legacy compiler.\n"
    "            let plan = plan_categorical_absorbed_request(&request)?;\n"
    "            if self.vce == OlsVceKind::Clustered {\n"
    "                let requested_clustered = OlsClusteredRequestV1 {\n"
    "                    ols: request.clone(),\n"
    "                    clusters: clusters.clone(),\n"
    "                };\n"
    "                validate_cluster_request_shape(&requested_clustered)?;\n"
    "                OlsClusteredRequestV1 {\n"
    "                    ols: plan.availability_request().clone(),\n"
    "                    clusters: clusters.clone(),\n"
    "                }\n"
    "                .validate_and_compile()?\n"
    "            } else {\n"
    "                plan.availability_request().validate_and_compile()?\n"
    "            }\n"
    "        } else if self.vce == OlsVceKind::Clustered {\n"
    "            let clustered_request = OlsClusteredRequestV1 {\n"
    "                ols: request.clone(),\n"
    "                clusters: clusters.clone(),\n"
    "            };\n"
    "            // This validates cluster-only sources without reading observations.\n"
    "            validate_cluster_request_shape(&clustered_request)?;\n"
    "            clustered_request.validate_and_compile()?\n"
    "        } else {\n"
    "            request.validate_and_compile()?\n"
    "        };\n",
    "hidden categorical pre-read validation",
)
ESTIMATE.write_text(estimate, encoding="utf-8")

contract = CONTRACT.read_text(encoding="utf-8")
contract = replace_once(
    contract,
    "        self.assertIn(\"categorical_sources.len() > 1\", estimate)\n",
    "        self.assertIn(\"categorical_sources.len() > 1\", estimate)\n"
    "        self.assertIn(\"plan_categorical_absorbed_request\", estimate)\n"
    "        self.assertIn(\"plan.availability_request().validate_and_compile()\", estimate)\n"
    "        self.assertIn(\"validate_cluster_request_shape(&requested_clustered)\", estimate)\n",
    "categorical pre-read source contract",
)
CONTRACT.write_text(contract, encoding="utf-8")

print("activated categorical hidden estimate pre-read validation")
