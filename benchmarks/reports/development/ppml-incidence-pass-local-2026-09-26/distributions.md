# Ordered distributions and timing adjudication

B=residualrows2; C=incidencepass1; S=pinned comparable stock. Times are ms.
These complete pilot/fresh blocks have clean monitors. Scientific discrepancies are unranked.

## development

|Screen/case; design; N/K/T; mode|B|C|S|C/S range|
|---|---|---|---|---|
|pilot/0; strong_d6; 983040/6/8; low|934;934;929|929;926;924|864;859;865|1.0682–1.0780|
|pilot/1; strong_d6; 983040/6/8; fast|858;859;871|858;859;856|862;870;861|0.9874–0.9954|
|pilot/2; strong_d6; 30720/6/4; low|70;71;70|70;70;70|122;120;117|0.5738–0.5983|
|pilot/3; three_fe_dense; 61440/10/8; fast|151;148;154|151;151;150|231;231;231|0.6494–0.6537|
|pilot/4; three_fe_dense; 153600/19/8; fast|570;570;568|572;567;567|716;691;697|0.7989–0.8205|
|pilot/5; strong_d6; 245760/6/1; fast|536;540;539|538;535;539|925;936;929|0.5716–0.5816|

B/C paired outcomes: {'wins': 9, 'losses': 3, 'ties': 6}. Families with every comparable pair <=0.90: 4.

## fresh

|Screen/case; design; N/K/T; mode|B|C|S|C/S range|
|---|---|---|---|---|
|fresh/0; strong_d6; 737280/6/8; fast|655;662;664;664;670|651;659;653;655;660|762;763;754;765;760|Unranked: scientific discrepancy|
|fresh/1; strong_d6; 737280/6/8; low|721;720;707;708;705|698;707;706;702;704|773;748;756;760;761|Unranked: scientific discrepancy|
|fresh/2; strong_d6; 368640/6/4; fast|417;415;414;414;415|406;409;411;409;410|661;670;659;655;662|0.6104–0.6244|
|fresh/3; three_fe_dense; 184320/19/8; fast|712;702;709;719;720|712;715;720;709;720|3131;3110;3149;3134;3156|0.2262–0.2299|
|fresh/4; three_fe_dense; 76800/10/8; fast|191;195;195;187;195|188;187;195;190;191|300;296;297;294;296|0.6267–0.6566|
|fresh/5; three_fe_dense; 38400/6/8; low|115;117;114;114;117|115;117;119;112;114|114;118;117;115;119|0.9580–1.0171|
|fresh/6; strong_d6; 245760/6/1; fast|544;554;541;540;538|541;540;542;537;538|935;933;935;922;939|0.5729–0.5824|
|fresh/7; weak_d3; 61440/6/4; fast|166;161;161;160;163|160;160;162;160;160|817;821;819;821;821|0.1949–0.1978|
|fresh/8; strong_d6; 61440/6/4; low|92;93;92;92;94|94;95;94;92;94|100;99;99;100;98|0.9200–0.9596|

B/C paired outcomes: {'wins': 27, 'losses': 9, 'ties': 9}. Families with every comparable pair <=0.90: 5.

## Protected OLS

|Case|B|C|
|---|---|---|
|0|67;64;67|64;69;64|
|1|75;80;75|77;75;76|
|2|15;16;14|14;14;14|
|3|207;209;206|207;212;207|
|4|160;160;157|159;158;158|
|5|17;17;17|17;17;17|

## Bounded unchanged control repeat

Same source, seed202609447 and exact full/warm numeric input arrays. Both original and repeated blocks are retained.

|Control|B|C|
|---|---|---|
|3|718;726;715;705;712|709;722;709;714;716|
|8|92;90;94;93;92|92;94;93;94;98|
