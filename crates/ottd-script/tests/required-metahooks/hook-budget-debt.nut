local d={_get=function(k){local n=0;
 for(local i=0;
i<8;
i+=1){n+=i;
} return n;
}};
 local t={};
 delegate d:t;
 assert(t.x==28);
 return 1;

