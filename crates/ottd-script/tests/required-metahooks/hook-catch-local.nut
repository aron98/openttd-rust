local d={_get=function(k){try{throw [7];
}catch(e){return e[0];
}}};
 local t={};
 delegate d:t;
 assert(t.x==7);
 return 1;

