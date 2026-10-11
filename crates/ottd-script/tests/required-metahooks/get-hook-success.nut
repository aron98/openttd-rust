local d={_get=function(k){::assert(k=="x");
 return 7;
}};
 local t={};
 delegate d:t;
 assert(t.x==7);
 return 1;

