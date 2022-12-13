set term png size 1200,800; # Width, Height
set output fileout;
set key noautotitle;

# Show two plots in 2 rows, 1 column;
set multiplot layout 2, 1 ;

# Scatter plot
set title charge . ": Time vs Gas";
set xlabel "Time (nanos)";
set ylabel "Gas (millis)";
plot filein with points pt 1;

# Time Histogram

# N buckets of equal length from 0 to the maximum time
n=100 #number of intervals
width=max_elapsed/n
hist(x,width)=width*floor(x/width)+width/2.0

set boxwidth width*0.9
set style fill solid 0.5
set title charge . ": Time Distribution";
set xlabel "Time (nanos)";
set ylabel "Frequency";
plot filein using (hist($1,width)):(1.0) smooth freq with boxes lc rgb"green" notitle;

unset multiplot
