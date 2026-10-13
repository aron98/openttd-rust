local d={_get=function(k){throw "bad";}};
local t=delegate d:{x=1};
assert(t.x==1);
return 1;
