* Test-only matched-probe reference. Row operators are staged verbatim from
* the frozen Separations reference. cached=0 repeats its cholsolve strategy;
* cached=1 uses one factor with Stata's triangular solve primitives.
capture mata: mata drop pb_solve()
capture mata: mata drop pb_correction()
mata:
real colvector pb_solve(real matrix information, real matrix factor, real colvector rhs, real scalar cached)
{
    if(cached) return(solveupper(factor',solvelower(factor,rhs)))
    return(cholsolve(information,rhs))
}
real matrix pb_correction(real colvector g,real colvector h,real matrix C,
    real colvector y,real colvector mu,real colvector weight,real colvector unit,
    real matrix signs,real scalar probes,real scalar cached)
{
    real matrix information,factor,result
    real colvector fc,fd,z,xi,d,c,dd,dc
    real scalar ng,nh,dimension,groups,group,count,first,probe,mass,next
    ng=max(g);nh=max(h);dimension=ng+nh+cols(C)
    information=ev__info(g,h,C,weight:*mu,ng,nh)
    factor=cached?cholesky(information):J(0,0,.)
    assert(!any(information:>=.));if(cached)assert(!any(factor:>=.))
    groups=min((8,probes));first=1;mass=0;result=J(dimension,5,0)
    for(group=1;group<=groups;group++) {
        count=floor(probes/groups)+(group<=mod(probes,groups))
        fc=fd=J(rows(y),1,0)
        for(probe=first;probe<first+count;probe++) {
            xi=signs[unit,probe]
            z=ev__xb(g,h,C,pb_solve(information,factor,
                ev__xt(g,h,C,xi:*weight:*(y:-mu),ng,nh),cached),ng,nh)
            fc=fc+z:^2/2;fd=fd+xi:*z
        }
        d=pb_solve(information,factor,ev__xt(g,h,C,weight:*mu:*fd/count,ng,nh),cached)
        c=pb_solve(information,factor,ev__xt(g,h,C,weight:*mu:*fc/count,ng,nh),cached)
        next=mass+count;dd=d-result[.,1];dc=c-result[.,2]
        result[.,1]=result[.,1]+count/next*dd
        result[.,2]=result[.,2]+count/next*dc
        result[.,3]=result[.,3]+count*mass/next*dd:^2
        result[.,4]=result[.,4]+count*mass/next*dc:^2
        result[.,5]=result[.,5]+count*mass/next*dd:*dc
        mass=next;first=first+count
    }
    result[.,3..5]=result[.,3..5]/(probes*(groups-1))
    return(result)
}
end

capture mata: mata drop pb_publish()
mata:
void pb_publish(real matrix result,real colvector g,real colvector h,
    real colvector rawg,real colvector rawh)
{
    real matrix values
    real colvector raw,v,ix
    string rowvector suffix
    real scalar effect,channel
    suffix=("raw","d","c","total","bc","sed","sec","covdc","setotal")
    for(effect=1;effect<=3;effect++) {
        if(effect==1) {
            values=result[g,.];raw=rawg
        }
        if(effect==2) {
            values=J(rows(g),5,0);ix=selectindex(h:>0)
            values[ix,.]=result[max(g):+h[ix],.];raw=rawh
        }
        if(effect==3) {
            values=result[(max(g)+max(h)+1)..rows(result),.] \ J(1,5,0)
            raw=st_matrix("e(b)")'
        }
        for(channel=1;channel<=9;channel++) {
            if(channel==1) v=raw
            if(channel==2) v=values[.,1]
            if(channel==3) v=values[.,2]
            if(channel==4) v=values[.,1]+values[.,2]
            if(channel==5) v=raw+values[.,1]+values[.,2]
            if(channel==6) v=sqrt(values[.,3])
            if(channel==7) v=sqrt(values[.,4])
            if(channel==8) v=values[.,5]
            if(channel==9) v=sqrt(rowmax((J(rows(values),1,0),values[.,3]+values[.,4]+2*values[.,5])))
            if(effect<3) st_store(.,st_addvar("double","bf_"+strofreal(effect)+"_"+suffix[channel]),v)
            else st_matrix("PB_bias_"+suffix[channel],v')
        }
    }
}
end
