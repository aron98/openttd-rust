local d={x=7};
 local t={};
 delegate d:t;
 local caught=false;
 try{delete t.x;
}catch(e){caught=true;
 assert(e=="the index 'x' does not exist");
} assert(caught);
 assert(d.x==7);
 return 1;

