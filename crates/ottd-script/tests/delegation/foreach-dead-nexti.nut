local d={_nexti=function(k){throw "bad";}};
local t=delegate d:{x=1,y=2};
local n=0;
foreach(k,v in t){n+=v;}
assert(n==3);
return 1;
