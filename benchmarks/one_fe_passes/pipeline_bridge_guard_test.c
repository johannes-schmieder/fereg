/* The integrated batch path: reordered variables, nonzero offset, tail and error. */
#include "stata_bridge.h"
#include <assert.h>
#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
static int calls, fail_at;
static pthread_t caller;
static ST_int nobs(void) { assert(pthread_equal(caller,pthread_self())); return 65540; }
static ST_int nvars(void) { assert(pthread_equal(caller,pthread_self())); return 52; }
static ST_boolean isstr(ST_int v) { assert(pthread_equal(caller,pthread_self())); assert(v>=1 && v<=52); return 0; }
static ST_int read_value(ST_int v,ST_int o,ST_double *d) {
    assert(pthread_equal(caller,pthread_self()));
    assert(v>=1 && v<=52 && o>=1 && o<=65540 && d);
    if (++calls==fail_at) return 919;
    *d=(double)(v*100000+o); return 0;
}
ST_int fereg_dispatch(ST_int argc,const char *const *argv) { (void)argc; (void)argv; return 0; }
int main(void) {
    caller=pthread_self(); ST_plugin plugin={0};
    plugin.nobs=nobs; plugin.nvars=nvars; plugin.isstr=isstr;
    plugin.safevdata=read_value; plugin.vdata=read_value; plugin.missval=0x1p+1023;
    assert(pginit(&plugin)==SD_PLUGINVER); assert(stata_call(0,NULL)==0);
    double *columns[52], missing=0; int variables[52];
    for (int j=0;j<52;++j) { variables[j]=52-j; columns[j]=malloc(16417*sizeof(double)); assert(columns[j]); }
    assert(fereg_spi_vdata_columns(variables,52,49124,16417,columns,&missing)==0);
    assert(calls==52*16417 && missing==plugin.missval);
    for (int j=0;j<52;++j) for (int i=0;i<16417;++i) assert(columns[j][i]==(double)(variables[j]*100000+49124+i));
    const int failures[]={1,9000,52*4096+1,52*16417};
    for (size_t f=0;f<sizeof(failures)/sizeof(failures[0]);++f) {
        calls=0; fail_at=failures[f];
        assert(fereg_spi_vdata_columns(variables,52,49124,16417,columns,&missing)==919);
        assert(calls==fail_at);
    }
    for (int j=0;j<52;++j) free(columns[j]);
    puts("PIPELINE_BRIDGE_GUARD_PASS"); return 0;
}
