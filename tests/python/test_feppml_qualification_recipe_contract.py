from __future__ import annotations
import copy, importlib.util
from pathlib import Path
import tempfile, unittest

ROOT = Path(__file__).resolve().parents[2]
def load(name, path):
    spec=importlib.util.spec_from_file_location(name,path); module=importlib.util.module_from_spec(spec); spec.loader.exec_module(module); return module
CONTRACT=load("q_contract",ROOT/"scripts/feppml_qualification_recipe_contract.py")
RECIPES=load("q_recipes_contract_test",ROOT/"scripts/generate_feppml_qualification_recipes.py")
ORACLE=load("q_oracle_contract_test",ROOT/"scripts/generate_feppml_oracle_fixtures.py")
CONSTRAINED=load("q_constrained_contract_test",ROOT/"scripts/generate_feppml_constrained_recipes.py")
MANIFEST=ROOT/"benchmarks/manifests/ppml_memory_performance_v1.json"

class QualificationRecipeContractTests(unittest.TestCase):
    def test_profile_is_stable_and_rejects_coordinated_mutations(self):
        profile=CONTRACT.canonical_profile(MANIFEST)
        self.assertEqual(len(profile["cells"]),33); self.assertEqual(len(profile["oracle"]),7)
        self.assertEqual(len(CONTRACT.profile_digest(profile)),64)
        with tempfile.TemporaryDirectory() as a, tempfile.TemporaryDirectory() as b, tempfile.TemporaryDirectory() as c:
            recipes=RECIPES.compile_recipes(MANIFEST,Path(a)); fixtures=ORACLE.compile_fixtures(MANIFEST,Path(b)); constrained=CONSTRAINED.compile_recipes(MANIFEST,Path(c))
            CONTRACT.validate_documents(profile,recipes,fixtures,constrained)
            CONTRACT.validate_executable_sources(profile,recipes,Path(a))
            CONTRACT.validate_oracle_models(profile, fixtures, Path(b))
            CONTRACT.validate_generator_sources(profile, recipes, Path(a), fixtures, Path(b))
            CONTRACT.validate_constrained_sources(profile, constrained, Path(c))
            mutations=[]
            pweight=copy.deepcopy(recipes); target=next(x for x in pweight["cells"] if x["commands"]["candidate_timing"].find("pweight(pw)")>=0); target["commands"]={k:v.replace(" pweight(pw)","").replace(" [pw=pw]","") for k,v in target["commands"].items()}; mutations.append((pweight,fixtures,constrained))
            tiny=copy.deepcopy(recipes); tiny["cells"][0]["n"]=256; mutations.append((tiny,fixtures,constrained))
            vce=copy.deepcopy(recipes); vce["cells"][0]["commands"]["candidate_timing"] += " vce(cluster cluster1)"; mutations.append((vce,fixtures,constrained))
            output=copy.deepcopy(recipes); output["cells"][0]["expected_output"]="prediction"; mutations.append((output,fixtures,constrained))
            status=copy.deepcopy(recipes); status["cells"][0]["expected_status_map"][0]["status_code"]=6; mutations.append((status,fixtures,constrained))
            rows=copy.deepcopy(fixtures); rows["cells"][0]["row_count"]=192; mutations.append((recipes,rows,constrained))
            for documents in mutations:
                with self.assertRaises(CONTRACT.QualificationRecipeContractError): CONTRACT.validate_documents(profile,*documents)
            import json
            pweight_record = next(record for expected, record in zip(profile["oracle"], fixtures["cells"]) if expected["weight"] == "pweight")
            model_path = Path(b) / pweight_record["model"]
            model = json.loads(model_path.read_text(encoding="ascii")); model["weight"]={"kind":"none","column":None}
            model_path.chmod(0o644); model_path.write_text(json.dumps(model),encoding="ascii")
            with self.assertRaises(CONTRACT.QualificationRecipeContractError): CONTRACT.validate_oracle_models(profile,fixtures,Path(b))

    def test_executable_science_and_timing_bytes_are_independently_frozen(self):
        profile=CONTRACT.canonical_profile(MANIFEST)
        with tempfile.TemporaryDirectory() as a:
            root=Path(a)
            recipes=RECIPES.compile_recipes(MANIFEST,root)
            CONTRACT.validate_executable_sources(profile,recipes,root)
            record=next(cell for cell in recipes["cells"] if "pweight(pw)" in cell["commands"]["candidate_timing"])
            for key in ("candidate_science_do","candidate_timing_do","comparator_science_do","comparator_timing_do"):
                path=root/record[key]
                original=path.read_text(encoding="ascii")
                path.chmod(0o644)
                stripped=original.replace(" pweight(pw)","").replace(" [pw=pw]","")
                self.assertNotEqual(stripped,original)
                path.write_text(stripped,encoding="ascii")
                with self.subTest(key=key), self.assertRaises(CONTRACT.QualificationRecipeContractError):
                    CONTRACT.validate_executable_sources(profile,recipes,root)
                path.write_text(original,encoding="ascii")
            path=root/record["candidate_timing_do"]
            original=path.read_text(encoding="ascii")
            path.write_text(original+"quietly count\n",encoding="ascii")
            with self.assertRaises(CONTRACT.QualificationRecipeContractError):
                CONTRACT.validate_executable_sources(profile,recipes,root)
            path.write_text(original,encoding="ascii")
            extra=path.parent/"extra.do"
            extra.write_text("version 19.0\n",encoding="ascii")
            with self.assertRaises(CONTRACT.QualificationRecipeContractError):
                CONTRACT.validate_executable_sources(profile,recipes,root)
            extra.unlink()
            altered=copy.deepcopy(recipes)
            altered_record=next(cell for cell in altered["cells"] if cell["cell_id"]==record["cell_id"])
            altered_record["candidate_timing_do"]=altered_record["candidate_science_do"]
            with self.assertRaises(CONTRACT.QualificationRecipeContractError):
                CONTRACT.validate_executable_sources(profile,altered,root)

    def test_constant_pweight_generators_are_rejected_in_both_domains(self):
        profile=CONTRACT.canonical_profile(MANIFEST)
        with tempfile.TemporaryDirectory() as a, tempfile.TemporaryDirectory() as b, tempfile.TemporaryDirectory() as c:
            recipes=RECIPES.compile_recipes(MANIFEST,Path(a)); fixtures=ORACLE.compile_fixtures(MANIFEST,Path(b)); CONSTRAINED.compile_recipes(MANIFEST,Path(c))
            main_record=next(record for expected,record in zip(profile["cells"],recipes["cells"]) if expected["weight"]=="pweight" and expected["controls"]>0)
            oracle_record=next(record for expected,record in zip(profile["oracle"],fixtures["cells"]) if expected["weight"]=="pweight" and expected["controls"]>0)
            for root, record in ((Path(a), main_record), (Path(b), oracle_record)):
                path=root/record["generator"]
                original=path.read_text(encoding="ascii")
                path.chmod(0o644)
                for override in ("replace pw = 1", "replace fw = 1", "replace offset = 0", "replace exposure = 1", "replace y = 1", "replace __mu = 1", "replace c1 = 0", "quietly replace pw = 1", "replace pw=1", "recode pw (else=1)", "#delimit ; replace pw=1; #delimit cr", "* dead-code comment", "display 1"):
                    path.write_text(original + override + "\n",encoding="ascii")
                    with self.subTest(root=root.name,override=override), self.assertRaises(CONTRACT.QualificationRecipeContractError):
                        CONTRACT.validate_generator_sources(profile,recipes,Path(a),fixtures,Path(b))
                path.write_text(original,encoding="ascii")

    def test_constrained_command_and_wrapper_mutations_are_rejected(self):
        profile=CONTRACT.canonical_profile(MANIFEST)
        with tempfile.TemporaryDirectory() as c:
            constrained=CONSTRAINED.compile_recipes(MANIFEST,Path(c))
            path=Path(c)/constrained["fixtures"][0]["attempts"][0]["do_file"]
            original=path.read_text(encoding="ascii"); path.chmod(0o644)
            for mutation in (
                original.replace("memory(low)","memory(auto)"),
                original+"quietly count\n",
                original.replace('file write __resolution "ADO|`__q_resolved_ado\'" _n\n', ""),
                original.replace("DEPENDENCY|`__q_resolved_parse'", "DEPENDENCY|/tmp/stale/fereg_parse.ado"),
            ):
                path.write_text(mutation,encoding="ascii")
                with self.assertRaises(CONTRACT.QualificationRecipeContractError): CONTRACT.validate_constrained_sources(profile,constrained,Path(c))
            path.write_text(original,encoding="ascii")

if __name__=="__main__": unittest.main()
