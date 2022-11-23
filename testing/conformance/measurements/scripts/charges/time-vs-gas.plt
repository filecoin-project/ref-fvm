set term png size 1200,800;
set output fileout;
set xlabel "Time (nanos)";
set ylabel "Gas (millis)";
set key noautotitle;

set title charge . ": Time vs Gas";
plot filein with points pt 1;
