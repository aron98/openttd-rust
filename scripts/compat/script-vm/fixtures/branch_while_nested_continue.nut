local i=0,s=0; while(i<3){i=i+1; local j=0; while(j<3){j=j+1; if(j==2) continue; s=s+1;} if(i==2) continue; s=s+10;} return s;
