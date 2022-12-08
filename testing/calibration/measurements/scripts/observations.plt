set term png size 1200,800;
set output fileout;

set title title . ": Size vs Time & Gas";
set xlabel "Size (bytes)";
set ylabel "Time (nanos)";
set y2label "Gas (millis)";
set y2tics;
set ytics nomirror;
set key outside;

plot for [i=0:*] filein index i using 1:2 with points pointtype 1 axis x1y1 title sprintf("%s Time", word(series, i+1)), \
     for [i=0:*] filein index i using 1:3 with points pointtype 5 axis x1y2 title sprintf("%s Gas",  word(series, i+1))
