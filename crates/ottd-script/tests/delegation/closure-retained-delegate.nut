local d={x=7};
 local t={};
 delegate d:t;
 local f=function():(t){return t.x;
};
 d=null;
 t=null;
 assert(f()==7);
 return 1;

