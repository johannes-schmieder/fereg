# Ordered distributions and timing adjudication

B=directstart1; C=classifiedinput1; S=pinned comparable stock. Times are ms.
These complete pilot/fresh blocks have clean monitors. Scientific discrepancies are unranked.

## development

|Screen/case; design; N/K/T; mode|B|C|S|C/S range|
|---|---|---|---|---|
|pilot/0; strong_d6; 983040/6/8; fast|897;902;904|887;875;881|859;860;865|1.0174–1.0326|
|pilot/1; strong_d6; 983040/6/8; low|1004;1003;1000|977;982;987|869;871;873|1.1243–1.1306|
|pilot/2; three_fe_dense; 153600/19/8; fast|576;590;579|571;571;574|708;710;701|0.8042–0.8188|
|pilot/3; three_fe_dense; 61440/10/8; fast|157;155;153|152;153;151|232;231;226|0.6552–0.6681|
|pilot/4; strong_d6; 30720/6/4; low|73;73;74|72;72;72|120;120;114|0.6000–0.6316|
|pilot/5; strong_d6; 245760/6/1; fast|548;548;549|543;549;541|935;938;940|0.5755–0.5853|
|pilot/6; three_fe_dense; 30720/6/4; fast|72;73;72|73;71;73|81;76;82|0.8902–0.9342|
|pilot/7; one_fe; 61440/6/4; fast|60;61;64|61;63;59|38;41;41|Unranked: scientific discrepancy|

B/C paired outcomes: {'wins': 19, 'losses': 5, 'ties': 0}. Families with every comparable pair <=0.90: 4.

## fresh

|Screen/case; design; N/K/T; mode|B|C|S|C/S range|
|---|---|---|---|---|
|fresh/0; strong_d6; 737280/6/8; fast|688;688;688;683;697|665;670;672;674;665|760;752;761;759;761|0.8739–0.8910|
|fresh/1; strong_d6; 737280/6/8; low|766;769;761;766;765|744;742;747;740;742|758;761;764;754;762|0.9738–0.9815|
|fresh/2; strong_d6; 368640/6/4; fast|431;427;429;430;428|418;418;420;419;418|669;656;656;654;670|0.6239–0.6407|
|fresh/3; three_fe_dense; 184320/19/8; fast|696;696;692;691;694|676;677;683;675;678|3204;3197;3192;3151;3177|0.2110–0.2142|
|fresh/4; three_fe_dense; 76800/10/8; fast|190;195;195;193;201|190;192;191;192;196|300;300;300;297;297|0.6333–0.6599|
|fresh/5; three_fe_dense; 38400/6/8; low|116;120;120;118;123|118;117;117;113;116|112;116;117;115;117|0.9826–1.0536|
|fresh/6; strong_d6; 245760/6/1; fast|547;546;548;549;552|543;542;545;540;541|938;935;949;945;963|0.5618–0.5797|
|fresh/7; weak_d3; 61440/6/4; fast|165;163;163;162;162|172;163;161;163;161|832;826;834;831;832|0.1930–0.2067|

B/C paired outcomes: {'wins': 35, 'losses': 3, 'ties': 2}. Families with every comparable pair <=0.90: 6.

## Protected OLS

|Case|B|C|
|---|---|---|
|0|66;67;65|67;67;70|
|1|77;77;78|76;77;76|
|2|14;15;14|15;15;15|
|3|210;207;209|206;206;211|
