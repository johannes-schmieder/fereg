# Ordered distributions and timing adjudication

B=objectiveblocks1; C=directionalscore2; S=pinned comparable stock. Times are ms.
These complete pilot/fresh blocks have clean monitors. Scientific discrepancies are unranked.

## development

|Screen/case; design; N/K/T; mode|B|C|S|C/S range|
|---|---|---|---|---|
|pilot/0; strong_d6; 983040/6/8; low|1035;958;930|937;984;920|932;931;892|1.0054–1.0569|
|pilot/1; strong_d6; 983040/6/8; fast|850;858;859|845;843;851|869;868;868|0.9712–0.9804|
|pilot/2; strong_d6; 30720/6/4; low|73;72;71|70;69;73|115;123;118|0.5610–0.6186|
|pilot/3; three_fe_dense; 61440/10/8; fast|154;152;151|149;144;155|234;233;234|0.6180–0.6624|
|pilot/4; three_fe_dense; 153600/19/8; fast|571;582;562|567;565;563|712;714;720|0.7819–0.7963|
|pilot/5; strong_d6; 245760/6/1; fast|543;545;546|547;548;547|954;953;943|0.5734–0.5801|

B/C paired outcomes: {'wins': 11, 'losses': 7, 'ties': 0}. Families with every comparable pair <=0.90: 4.

## fresh

|Screen/case; design; N/K/T; mode|B|C|S|C/S range|
|---|---|---|---|---|
|fresh/0; strong_d6; 737280/6/8; fast|657;643;646;653;652|633;645;640;643;646|768;761;759;767;756|0.8242–0.8545|
|fresh/1; strong_d6; 737280/6/8; low|704;708;1107;708;708|708;705;713;698;690|768;769;756;765;766|0.9008–0.9431|
|fresh/2; strong_d6; 368640/6/4; fast|456;453;454;453;455|448;450;447;445;446|652;662;661;665;650|0.6692–0.6871|
|fresh/3; three_fe_dense; 184320/19/8; fast|689;686;681;686;686|679;684;679;678;671|3163;3160;3088;3157;3022|0.2147–0.2220|
|fresh/4; three_fe_dense; 76800/10/8; fast|192;189;191;196;192|185;189;184;192;191|292;291;297;291;295|0.6195–0.6598|
|fresh/5; three_fe_dense; 38400/6/8; low|112;113;119;115;124|113;115;112;111;112|116;115;117;120;117|0.9250–1.0000|
|fresh/6; strong_d6; 245760/6/1; fast|541;540;539;539;538|538;541;541;545;541|925;945;940;941;943|0.5725–0.5816|
|fresh/7; weak_d3; 61440/6/4; fast|121;120;124;124;120|121;121;119;120;119|806;842;839;837;837|0.1418–0.1501|
|fresh/8; strong_d6; 61440/6/4; low|93;94;92;93;95|92;94;91;93;93|98;98;98;98;99|0.9286–0.9592|

B/C paired outcomes: {'wins': 32, 'losses': 9, 'ties': 4}. Families with every comparable pair <=0.90: 6.

## Protected OLS

|Case|B|C|
|---|---|---|
|0|64;65;65|63;63;65|
|1|76;77;76|77;76;79|
|2|12;14;14|14;15;15|
|3|208;208;208|216;211;210|
|4|160;157;160|160;158;157|
|5|17;17;18|18;18;18|

## Fixed-input follow-ups

|Screen/original case|B ordered ms|C ordered ms|Paired wins/losses/ties|
|---|---|---|---|
|serial-repeat/5|535;536;542;554;540|540;543;545;541;541|1/4/0|
|ols-repeat/2|14;11;14;12;15|14;14;14;14;11|1/2/2|
|ols-repeat/3|208;213;220;209;209|207;208;211;209;211|3/1/1|
|ols-repeat/5|17;18;18;17;17|17;17;18;18;18|1/2/2|
