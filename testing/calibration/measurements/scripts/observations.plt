set term png size 1200,800;
set output fileout;

set title title . ": Size vs Time & Gas";
set xlabel "Size (bytes)";
set ylabel "Time (nanos)";
set key outside;

# "Gas (millis)" is converted to a time equivalent by the expectation of 10 Gas/nanos.

plot for [i=0:*] filein index i using 1:2          with points title sprintf("%s Time", word(series, i+1)), \
     for [i=0:*] filein index i using 1:($3/10000) with lines  title sprintf("%s Gas to time",  word(series, i+1))
