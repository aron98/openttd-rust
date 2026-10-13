local d={get=function(){return this.x;
}};
 local t={x=9};
 delegate d:t;
 assert(t.get()==9);
 return 1;

