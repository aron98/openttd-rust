local o=::__owner_setup[0];
local f=function():(o){
    ::__owner_setup=null;
    ::f=null;
    ::print("collect-active");
    ::assert(o[0]==o);
    return 1;
};
::f <- f;
o=null;
f=null;
return ::f();
