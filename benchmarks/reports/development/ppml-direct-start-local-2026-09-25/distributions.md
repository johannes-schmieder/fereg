# Ordered distributions and timing adjudication

B=familyframe1; C=directstart1; S=pinned comparable stock. Times are ms.
These complete pilot-repeat/fresh blocks have clean monitors. Scientific discrepancies are unranked. The interrupted original pilot is retained separately.

## development

|Screen/case; design; N/K/T; mode|B|C|S|C/S range|
|---|---|---|---|---|
|pilot-repeat/0; strong_d6; 983040/6/8; fast|941;948;945|922;914;912|880;868;866|1.0477–1.0531|
|pilot-repeat/1; strong_d6; 983040/6/8; low|1044;1043;1048|1015;1007;1012|867;870;868|1.1575–1.1707|
|pilot-repeat/2; three_fe_dense; 153600/19/8; fast|584;583;584|581;581;586|705;709;712|0.8195–0.8241|
|pilot-repeat/3; three_fe_dense; 61440/10/8; fast|156;157;156|152;161;162|236;231;230|0.6441–0.7043|
|pilot-repeat/4; strong_d6; 30720/6/4; low|76;74;73|73;74;75|119;118;119|0.6134–0.6303|
|pilot-repeat/5; strong_d6; 245760/6/1; fast|560;565;561|553;555;552|942;943;949|0.5817–0.5885|
|pilot-repeat/6; three_fe_dense; 30720/6/4; fast|74;74;74|73;74;75|82;82;81|0.8902–0.9259|
|pilot-repeat/7; one_fe; 61440/6/4; fast|60;65;61|61;60;61|43;43;40|Unranked: scientific discrepancy|

B/C paired outcomes: {'wins': 15, 'losses': 6, 'ties': 3}. Families with every comparable pair <=0.90: 4.

## fresh

|Screen/case; design; N/K/T; mode|B|C|S|C/S range|
|---|---|---|---|---|
|fresh/0; strong_d6; 737280/6/8; fast|723;717;711;720;714|691;692;694;697;688|763;758;761;771;755|Unranked: scientific discrepancy|
|fresh/1; strong_d6; 737280/6/8; low|795;791;794;795;789|765;766;769;767;765|756;757;766;760;762|Unranked: scientific discrepancy|
|fresh/2; strong_d6; 368640/6/4; fast|494;488;489;492;491|477;477;480;478;477|658;660;663;657;661|0.7216–0.7275|
|fresh/3; three_fe_dense; 184320/19/8; fast|696;697;698;694;695|681;684;686;685;684|3239;3221;3195;3222;3211|0.2103–0.2147|
|fresh/4; three_fe_dense; 76800/10/8; fast|198;200;199;202;200|198;197;191;195;194|292;302;298;296;291|0.6409–0.6781|
|fresh/5; three_fe_dense; 38400/6/8; low|119;120;120;118;118|121;119;117;120;118|119;119;118;119;121|0.9752–1.0168|
|fresh/6; strong_d6; 245760/6/1; fast|562;560;564;561;562|552;554;555;551;559|940;931;938;941;938|0.5855–0.5959|
|fresh/7; weak_d3; 61440/6/4; fast|126;127;124;128;126|126;125;123;124;123|852;857;853;855;857|0.1435–0.1479|

B/C paired outcomes: {'wins': 35, 'losses': 2, 'ties': 3}. Families with every comparable pair <=0.90: 5.

## Protected OLS

|Case|B|C|
|---|---|---|
|0|64;64;67|66;66;67|
|1|76;77;76|75;77;75|
|2|14;14;15|10;14;16|
|3|211;209;210|210;209;211|
