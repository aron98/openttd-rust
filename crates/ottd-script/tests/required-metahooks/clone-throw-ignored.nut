local d={_cloned=function(src){this.x=9;
 throw "clone";
}};
 local t={x=2};
 delegate d:t;
 local c=clone t;
 assert(c.x==9);
 assert(t.x==2);
 return 1;

