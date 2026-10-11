local o={};
local d={f=function(x=o){return x;}};
delegate d:o;
return o;
