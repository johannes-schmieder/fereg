import unittest
from outcomes import audited_outcome

class NativeOutcomeTests(unittest.TestCase):
    def test_numeric_and_boolean_nonconvergence_preserve_receipt(self):
        for flag in [False,0,0.0]:
            source={'status':'pass','metadata':{'rc':0,'converged':flag}}
            result=audited_outcome(source)
            self.assertEqual(result['status'],'nonconvergence')
            self.assertEqual(source['status'],'pass')
            self.assertEqual(result['recorded_status'],'pass')

    def test_positive_or_unreported_convergence_is_not_invented(self):
        for flag in [True,1,1.0,None]:
            self.assertEqual(audited_outcome({'status':'pass','metadata':{'converged':flag}})['status'],'pass')

    def test_failures_keep_measured_category_and_numerical_code(self):
        result=audited_outcome({'status':'worker_failure','metadata':{'rc':430,'converged':0}})
        self.assertEqual(result['status'],'worker_failure')
        self.assertEqual(result['failure_class'],'native_numerical_failure_r430')
        self.assertEqual(audited_outcome({'status':'command_timeout','metadata':{}})['status'],'command_timeout')

if __name__=='__main__':unittest.main()
