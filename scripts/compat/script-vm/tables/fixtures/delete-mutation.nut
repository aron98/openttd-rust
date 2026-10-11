local t=held;
local s="";
foreach(k,v in t){local c=clone t; s=s+k+":"+c[k]+","; delete t[k];}
return s;
