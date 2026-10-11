local d={x=7};
 local t={y=2};
 delegate d:t;
 local n=0;
 foreach(k,v in t){assert(k=="y");
 n+=v;
} assert(n==2);
 return 1;

