local f=function(){::print("collect-active");return 1;};
local g=function(next,object){
    ::__owner_setup=null;
    local a=1;
    local b=2;
    local c=3;
    local d=4;
    local e=5;
    local keep=object;
    return next();
};
return g(f,::__owner_setup[0]);
