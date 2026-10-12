local d={x=7};
 local t={y=2};
 delegate d:t;
 local c=clone t;
 assert(c!=t);
 assert(c.parent==d);
 c.y=8;
 assert(t.y==2);
 d.x=9;
 assert(c.x==9);
 return 1;

