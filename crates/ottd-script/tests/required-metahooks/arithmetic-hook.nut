local d={_add=function(v){return this.x+v;
}};
 local t={x=7};
 delegate d:t;
 assert(t+2==9);
 return 1;

