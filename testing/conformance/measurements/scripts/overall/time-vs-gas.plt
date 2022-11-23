set term png size 1200,800;
set output fileout;
set xlabel "Time (nanos)";
set ylabel "Gas (millis)";
set key noautotitle;

# Show two plots in 2 rows, 1 column;
set multiplot layout 2, 1 ;

set title "Overall Time vs Gas (over 1e9 gas)";
plot filein using ($1 < 1e8 ? $1 : 1/0):($2 > 1e9 ? $2 : 1/0) with points pt 1;

set title "Overall Time vs Gas (under 1e9 gas)";
plot filein using ($1 < 1e8 ? $1 : 1/0):($2 <= 1e9 ? $2 : 1/0) with points pt 1 ;

unset multiplot
