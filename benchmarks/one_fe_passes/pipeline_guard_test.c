#include "pipeline_probe.c"
#include <assert.h>
static int calls, fail_at, mock_n;
static pthread_t caller;
static ST_int observations(void) { assert(pthread_equal(caller,pthread_self())); return mock_n; }
static ST_int variables(void) { assert(pthread_equal(caller,pthread_self())); return 3; }
static ST_boolean is_string(ST_int v) { assert(pthread_equal(caller,pthread_self())); assert(v>=1 && v<=3); return 0; }
static ST_int value(ST_int v,ST_int o,ST_double *d) {
    assert(pthread_equal(caller,pthread_self()));
    assert(v>=1 && v<=3 && o>=1 && o<=mock_n && d);
    if (++calls==fail_at) return 919;
    *d=(double)(v*100000+o); return 0;
}
int main(void) {
    caller=pthread_self();
    ST_plugin plugin={0};
    plugin.nobs=observations; plugin.nvars=variables; plugin.isstr=is_string;
    plugin.safevdata=value; plugin.vdata=value;
    assert(pginit(&plugin)==SD_PLUGINVER);
    double *columns[3],*buffer=malloc(2*4096*3*sizeof(double));
    assert(buffer);
    for (int j=0;j<3;++j) { columns[j]=malloc(8193*sizeof(double)); assert(columns[j]); }
    const int sizes[]={1,255,256,257,1023,1024,1025,4095,4096,4097,8193};
    for (size_t n=0;n<sizeof(sizes)/sizeof(sizes[0]);++n) {
        mock_n=sizes[n];
        for (int mode=0;mode<8;++mode) {
            calls=0; fail_at=0;
            assert(read_values(mode,mock_n,3,columns,buffer)==0);
            assert(calls==mock_n*3);
            for (int j=0;j<3;++j) for (int i=0;i<mock_n;++i)
                assert(columns[j][i]==(double)((j+1)*100000+i+1));
            const int failures[]={1,mock_n*3/2+1,mock_n*3};
            for (size_t f=0;f<sizeof(failures)/sizeof(failures[0]);++f) {
                calls=0; fail_at=failures[f];
                assert(read_values(mode,mock_n,3,columns,buffer)==919);
                assert(calls==fail_at);
            }
        }
    }
    for (int j=0;j<3;++j) free(columns[j]); free(buffer);
    puts("PIPELINE_GUARD_PASS"); return 0;
}
