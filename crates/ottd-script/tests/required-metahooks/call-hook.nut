local d={_call=function(env,v){return v+3;
}};
 local t={};
 delegate d:t;
 assert(t(6)==9);
 return 1;

