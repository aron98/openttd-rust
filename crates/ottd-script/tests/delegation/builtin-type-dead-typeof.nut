local d={_typeof=function(){throw "bad";}};
local t=delegate d:{};
assert(type(t)=="table");
return 1;
