events<-{n=0};
 local d={};
 local t={};
 local left=function():(d){events.n=events.n*10+1;
 return d;
};
 local right=function():(t){events.n=events.n*10+2;
 return t;
};
 delegate left():right();
 assert(events.n==12);
 assert(t.parent==d);
 return 1;

