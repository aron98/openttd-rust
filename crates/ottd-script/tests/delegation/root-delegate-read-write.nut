local d={x=7};
 delegate d:this;
 assert(x==7);
 x=9;
 assert(d.x==9);
 delegate null:this;
 return 1;

