local d={_cmp=function(v){return this.x-v.x;
}};
 local t={x=7};
 local u={x=2};
 delegate d:t;
 assert(t>u);
 assert(t!=u);
 return 1;

