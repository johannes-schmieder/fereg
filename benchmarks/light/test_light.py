import sys
import tempfile
import unittest
from pathlib import Path

import numpy as np
import pandas as pd

sys.path.insert(0,str(Path(__file__).parent))
from common import ARMS, SIZES, cell, cell_id, manifest, supported, tasks
from adapters import stata_command
from inputs import generate
from memory import parse
from oracle import expected
from runner import fixture
from report import eligible, summarize
from followup_plan import select_cases


class LightTests(unittest.TestCase):
    def test_ten_bounded_bundles(self):
        tt=tasks()
        self.assertEqual(len(tt),10)
        self.assertTrue(all(len(t["cells"])==4 and t["max_fits"]==120 and t["wall_seconds"]==43200 for t in tt))
        ids=[cell_id(c) for t in tt for c in t["cells"]]
        self.assertEqual(len(set(ids)),40)
        self.assertEqual(sum(len(ARMS[c["model"]])*2*3 for t in tt for c in t["cells"]),1200)

    def test_explicit_feature_capabilities(self):
        c=dict(cell("weak_d3",SIZES[0],cluster=("g1","g2")),model="ols")
        self.assertFalse(supported(c,"greg")[0])
        self.assertTrue(supported(c,"fixest")[0])
        self.assertIn("vce(cluster g1 g2)",stata_command(c,"fereg",4))

    def test_saved_slope_command(self):
        c=dict(cell("weak_d3",SIZES[0],slopes=True,save=True),model="ppml")
        command=stata_command(c,"feppml",28)
        self.assertIn("saved2=g2##c.z",command)
        self.assertIn("mu(audit_mu)",command)
        self.assertNotIn("solver(",command)

    def test_poisson_gtools_captures_link_not_working_residual(self):
        c=dict(cell("strong_d3",360,k=2,save=True),model="ppml")
        command=stata_command(c,"gglm",4)
        self.assertIn("predict(audit_link)",command)
        self.assertNotIn("resid(",command)

    def test_width_holds_outcome_and_graph(self):
        with tempfile.TemporaryDirectory() as directory:
            frames=[]
            for k in (2,10,50):
                c=dict(cell("weak_d3",7680,k=k),model="ols")
                path=Path(directory)/f"k{k}.dta"
                generate(c,path)
                frames.append(pd.read_stata(path))
            for other in frames[1:]:
                np.testing.assert_array_equal(frames[0][["obs_id","g1","g2","y","x1","x2"]],other[["obs_id","g1","g2","y","x1","x2"]])

    def test_slope_is_not_period(self):
        with tempfile.TemporaryDirectory() as directory:
            c=dict(cell("weak_d3",7680,slopes=True),model="ols")
            path=Path(directory)/"slope.dta"
            generate(c,path)
            f=pd.read_stata(path)
            self.assertTrue((f.groupby("g2").z.var()>0).all())

    def test_pss_is_not_rss_and_requires_counters(self):
        text="Rss: 120 kB\nPss: 80 kB\nPrivate_Clean: 10 kB\nPrivate_Dirty: 30 kB\nSwap: 0 kB\n"
        self.assertEqual(parse(text)["Pss"],80)
        self.assertEqual(parse(text)["Uss"],40)
        with self.assertRaises(ValueError):
            parse("Rss: 120 kB\n")

    def test_dense_oracles_are_identified_and_finite(self):
        with tempfile.TemporaryDirectory() as directory:
            for model in ARMS:
                c=dict(cell("strong_d3",360,k=2,slopes=True),model=model)
                path=Path(directory)/(model+".dta")
                fixture(c,path)
                result=expected(path,c)
                self.assertTrue(np.isfinite(result["covariance"]).all())
                self.assertTrue((np.diag(result["covariance"])>0).all())

    def test_unresolved_reference_audits_exclude_ratios_except_adr26(self):
        call=dict(status="pass",arm="fixest",comparison={"status":"pass"},science={"status":"indeterminate"})
        self.assertFalse(eligible(call))
        self.assertTrue(eligible(dict(call,arm="ppmlhdfe")))
        self.assertFalse(eligible(dict(call,arm="ppmlhdfe",comparison={"status":"accuracy_or_evidence_failure"})))

    def test_ratio_is_median_of_pairs(self):
        c=dict(cell("one_fe",360,k=2),model="ols")
        calls=[]
        for rep,(a,b) in enumerate(((1,10),(4,2),(9,3)),1):
            for arm,value in (("fereg",a),("fixest",b)):
                calls.append(dict(cell=c,arm=arm,threads=4,repetition=rep,status="pass",seconds=value,
                                  science={"status":"pass"},comparison={"status":"pass"}))
        row=next(r for r in summarize({"calls":calls}) if r["arm"]=="fixest")
        self.assertEqual(row["seconds_ratio"],2)
        self.assertNotEqual(row["seconds_ratio"],4/3)

    def test_longer_followup_selects_only_ppml_timeouts_and_pairs(self):
        c=dict(cell("weak_d3",7680,k=2),model="ppml")
        base=dict(cell=c,threads=4,repetition=1,input_sha256="same",command_time_lower_bound=300)
        records=[(dict(base,arm="ppmlhdfe",status="command_timeout"),"timeout"),
                 (dict(base,arm="fixest",status="pass"),"success"),
                 (dict(base,arm="pyfixest",status="worker_failure"),"nonconvergence"),
                 (dict(base,cell=dict(c,model="ols"),arm="reghdfe",status="command_timeout"),"ols")]
        selected=select_cases(records)
        self.assertEqual(len(selected),1)
        self.assertEqual(selected[0]["arms"],["feppml","ppmlhdfe"])
        self.assertEqual(selected[0]["repetitions"],3)


if __name__=="__main__":
    unittest.main()
