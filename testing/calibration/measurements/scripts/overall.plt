set term png size 1200,800;
set output fileout;

set title "Time vs Gas";
set xlabel "Time (nanos)";
set ylabel "Gas (millis)";
set key outside;

plot for [i=0:*] filein index i using 1:2 with points pointtype 1 title sprintf("%s Time", word(series, i+1))
