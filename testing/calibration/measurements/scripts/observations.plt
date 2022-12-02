set term png size 1200,800;
set output fileout;
set xlabel "Size (bytes)";
set ylabel "Time (nanos)";
set key outside;

set title title . ": Time vs Input Size";

plot for [i=0:*] filein index i using 1:2 with points title word(series, i+1)
