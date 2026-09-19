/* Diagnostic transport only. SPI stays on the caller; the worker owns copies. */
#include "stplugin.h"
#include <pthread.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

struct tile { double *data; int first, count, ready; };
struct pipeline {
    pthread_mutex_t mutex;
    pthread_cond_t changed;
    struct tile tiles[2];
    double **columns;
    int k, stop;
};

static double seconds(void) {
    struct timespec t;
    if (clock_gettime(CLOCK_MONOTONIC, &t)) abort();
    return (double)t.tv_sec + (double)t.tv_nsec * 1e-9;
}
static void transpose(struct tile *tile, double **columns, int k) {
    for (int j=0; j<k; ++j) for (int i=0; i<tile->count; ++i)
        columns[j][tile->first+i] = tile->data[(size_t)i*(size_t)k+(size_t)j];
}
static void *consume(void *argument) {
    struct pipeline *p = argument;
    for (int slot=0;; slot=1-slot) {
        pthread_mutex_lock(&p->mutex);
        while (!p->tiles[slot].ready && !p->stop)
            pthread_cond_wait(&p->changed, &p->mutex);
        if (!p->tiles[slot].ready) { pthread_mutex_unlock(&p->mutex); break; }
        pthread_mutex_unlock(&p->mutex);
        transpose(&p->tiles[slot], p->columns, p->k);
        pthread_mutex_lock(&p->mutex);
        p->tiles[slot].ready=0;
        pthread_cond_broadcast(&p->changed);
        pthread_mutex_unlock(&p->mutex);
    }
    return NULL;
}
static int read_values(int mode, int n, int k, double **columns, double *buffer) {
    ST_IIIDp reader = mode == 0 ? _stata_->safevdata : _stata_->vdata;
    int rc=0;
    if (!reader || n<1 || n>SF_nobs() || k<1 || k>SF_nvars()) return 498;
    for (int j=0; j<k; ++j) if (!columns[j] || SF_var_is_string(j+1)) return 498;
    if (mode<2) {
        for (int i=0; i<n; ++i) for (int j=0; j<k; ++j) {
            rc=reader(j+1,i+1,&columns[j][i]);
            if (rc) return rc;
        }
        return 0;
    }
    int width = mode<4 ? 256 : mode<6 ? 1024 : 4096;
    int pipelined = mode%2;
    struct pipeline p = { .columns=columns, .k=k };
    p.tiles[0].data=buffer;
    p.tiles[1].data=buffer+(size_t)width*(size_t)k;
    pthread_t worker;
    if (pipelined) {
        if (pthread_mutex_init(&p.mutex,NULL)) return 909;
        if (pthread_cond_init(&p.changed,NULL)) { pthread_mutex_destroy(&p.mutex); return 909; }
        if (pthread_create(&worker,NULL,consume,&p)) {
            pthread_cond_destroy(&p.changed); pthread_mutex_destroy(&p.mutex); return 909;
        }
    }
    int slot=0;
    for (int first=0; first<n && !rc; first+=width,slot=1-slot) {
        struct tile *t=&p.tiles[slot];
        if (pipelined) {
            pthread_mutex_lock(&p.mutex);
            while (t->ready) pthread_cond_wait(&p.changed,&p.mutex);
            pthread_mutex_unlock(&p.mutex);
        }
        t->first=first; t->count=n-first<width ? n-first : width;
        for (int i=0; i<t->count && !rc; ++i) for (int j=0; j<k; ++j) {
            rc=reader(j+1,first+i+1,&t->data[(size_t)i*(size_t)k+(size_t)j]);
            if (rc) break;
        }
        if (rc) break;
        if (pipelined) {
            pthread_mutex_lock(&p.mutex);
            t->ready=1;
            pthread_cond_broadcast(&p.changed);
            pthread_mutex_unlock(&p.mutex);
        } else transpose(t,columns,k);
    }
    if (pipelined) {
        pthread_mutex_lock(&p.mutex);
        p.stop=1;
        pthread_cond_broadcast(&p.changed);
        pthread_mutex_unlock(&p.mutex);
        pthread_join(worker,NULL);
        pthread_cond_destroy(&p.changed);
        pthread_mutex_destroy(&p.mutex);
    }
    return rc;
}
static uint64_t hash_values(int n,int k,double **columns) {
    uint64_t h=UINT64_C(14695981039346656037);
    for (int i=0; i<n; ++i) for (int j=0; j<k; ++j) {
        uint64_t bits;
        memcpy(&bits,&columns[j][i],sizeof(bits));
        h=(h^bits)*UINT64_C(1099511628211);
    }
    return h;
}
STDLL stata_call(int argc,char *argv[]) {
    int n=SF_nobs(),k=SF_nvars(),rc=0;
    if (argc!=1 || n<1 || n>4000000 || k<1 || k>1024) return 198;
    double **columns=calloc((size_t)k,sizeof(*columns));
    double *buffer=malloc(2*4096*(size_t)k*sizeof(*buffer));
    FILE *out=NULL;
    if (!columns || !buffer) { rc=909; goto cleanup; }
    for (int j=0; j<k; ++j) {
        columns[j]=malloc((size_t)n*sizeof(**columns));
        if (!columns[j]) { rc=909; goto cleanup; }
    }
    out=fopen(argv[0],"w");
    if (!out) { rc=603; goto cleanup; }
    fprintf(out,"repeat\tposition\tmode\tseconds\thash\n");
    uint64_t expected=0;
    for (int rep=0; rep<11; ++rep) for (int position=0; position<8; ++position) {
        int mode=(position+rep)%8;
        for (int j=0; j<k; ++j) memset(columns[j],0,(size_t)n*sizeof(**columns));
        memset(buffer,0,2*4096*(size_t)k*sizeof(*buffer));
        double start=seconds();
        rc=read_values(mode,n,k,columns,buffer);
        double elapsed=seconds()-start;
        if (rc) goto cleanup;
        uint64_t h=hash_values(n,k,columns);
        if (!rep && !position) expected=h;
        if (h!=expected) { rc=9; goto cleanup; }
        fprintf(out,"%d\t%d\t%d\t%.9f\t%llu\n",rep,position,mode,elapsed,(unsigned long long)h);
        fflush(out);
    }
cleanup:
    if (out) fclose(out);
    if (columns) for (int j=0; j<k; ++j) free(columns[j]);
    free(columns); free(buffer);
    return rc;
}
