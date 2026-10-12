local f=function(...){
    ::__owner_setup=null;
    ::print("collect-active");
    ::assert(vargv[0][0]==vargv[0]);
    return 1;
};
return f(::__owner_setup[0]);
