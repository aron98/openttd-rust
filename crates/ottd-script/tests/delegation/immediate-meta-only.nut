local g={_typeof=function(){throw "bad";},_cloned=function(b){throw "bad";},_delslot=function(k){throw "bad";},_add=function(v){throw "bad";}};
local d=delegate g:{};
local t=delegate d:{x=3};
assert(typeof t=="table");
local c=clone t;
assert(c.parent==d && delete c.x==3);
try{t+1;
assert(false);}catch(e){assert(e=="arith op + on between 'table' and 'integer'");}
return 1;
