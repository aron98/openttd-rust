local d={_tostring=function(){return "ok\0α";
}};
 local t={};
 delegate d:t;
 assert((""+t)=="ok\0α");
 return 1;

