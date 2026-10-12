local o={};
local d={back=o};
delegate d:o;
try{throw o;}catch(e){}
return o;
