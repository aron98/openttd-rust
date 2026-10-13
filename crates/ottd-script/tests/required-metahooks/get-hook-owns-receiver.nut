local d={_get=function(k){return this.v+1;
}};
 local t={v=8};
 delegate d:t;
 assert(t.x==9);
 return 1;

