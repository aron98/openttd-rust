local d={_cmp=function(b){throw "bad";}};
local t=delegate d:{};
assert(t<=t && t>=t && !(t<t) && !(t>t) && t==t);
return 1;
