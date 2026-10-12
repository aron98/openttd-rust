local f=function(){
    ::__owner_setup=null;
    ::print("collect-active");
    return this[0]==this ? 1 : 0;
}.bindenv(::__owner_setup[0]);
return f();
