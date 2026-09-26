# Ordered uncontended distributions

B=canonicalthree1; C=implicitframe1; S=pinned comparable stock. Times are ms.
Only complete clean blocks below; all raw attempts remain in the archive. Unranked rows cannot support stock speed claims.

## development

|Screen/case; design; N/K/T; mode|B|C|S|C/S range|
|---|---|---|---|---|
|pilot2/0; strong_d6; 983040/6/8; fast|1252;1248;1246|1003;1019;1013|882;877;878|1.1372–1.1619|
|pilot2/1; strong_d6; 491520/6/8; fast|630;628;623|504;512;506|506;505;495|0.9960–1.0222|
|pilot2/2; strong_d6; 245760/6/4; fast|316;321;316|317;318;324|382;386;389|0.8238–0.8329|
|pilot2/3; strong_d6; 983040/6/8; low|1365;1357;1362|1100;1102;1119|875;859;865|1.2571–1.2936|
|pilot2/4; strong_d6; 491520/19/8; fast|1035;1030;1041|1039;1037;1035|1192;1173;1180|0.8716–0.8841|
|controls/0; three_fe_dense; 153600/19/8; fast|613;595;615|626;589;601|744;710;714|0.8296–0.8417|
|controls/1; three_fe_dense; 153600/19/8; low|528;541;527|536;544;526|710;696;702|0.7493–0.7816|
|controls/2; strong_d6; 30720/6/8; low|72;75;75|69;72;70|81;82;81|0.8519–0.8780|
|controls-rest/0; three_fe_dense; 38400/6/4; fast|248;248;255|243;245;248|10145;10214;10197|0.0240–0.0243|
|controls-rest/1; strong_d6; 153600/19/8; fast|416;425;422|424;432;425|518;523;515|0.8185–0.8260|

B/C paired outcomes: {'wins': 20, 'losses': 10, 'ties': 0}. Families with every comparable pair <=0.90: 7.

## fresh

|Screen/case; design; N/K/T; mode|B|C|S|C/S range|
|---|---|---|---|---|
|fresh/0; strong_d6; 737280/6/8; fast|976;928;932;990;932|762;764;756;762;762|756;763;755;759;762|1.0000–1.0079|
|fresh/1; strong_d6; 368640/6/4; fast|594;602;597;591;596|463;462;462;464;462|670;660;651;661;663|Unranked: scientific discrepancy|
|fresh-retry/0; strong_d6; 737280/6/8; low|1027;1021;1029;1031;1037|836;830;841;836;834|765;765;758;760;780|1.0692–1.1095|
|fresh-retry/1; strong_d6; 491520/19/8; fast|1360;1364;1369;1356;1356|1360;1364;1368;1365;1362|1394;1404;1382;1383;1382|Unranked: scientific discrepancy|
|fresh/4; strong_d6; 245760/6/1; fast|553;553;556;557;553|557;555;555;556;556|940;933;930;931;932|0.5926–0.5972|
|fresh/5; three_fe_dense; 184320/19/8; fast|764;762;762;754;755|756;762;754;760;760|3232;3249;3201;3234;3215|0.2339–0.2364|
|fresh/6; three_fe_dense; 38400/6/4; fast|190;186;190;187;181|183;186;193;201;186|13121;13100;13130;13133;13214|Unranked: scientific discrepancy|
|fresh/7; three_fe_dense; 38400/6/8; low|120;123;123;121;123|119;117;128;125;127|116;119;128;122;118|0.9832–1.0763|

B/C paired outcomes: {'wins': 23, 'losses': 13, 'ties': 4}. Families with every comparable pair <=0.90: 2.

## weak_diagnostic

|Screen/case; design; N/K/T; mode|B|C|S|C/S range|
|---|---|---|---|---|
|weak-screen/0; weak_d3; 61440/6/4; fast|162|135|812|0.1663–0.1663|
|weak-screen/1; weak_d3; 122880/10/8; low|411|277|1340|0.2067–0.2067|

B/C paired outcomes: {'wins': 2, 'losses': 0, 'ties': 0}. Families with every comparable pair <=0.90: 2.

## Protected OLS

|Case|B|C|
|---|---|---|
|0|66;69;64|70;64;66|
|1|77;76;84|75;78;74|
|2|14;14;14|14;14;15|
|3|214;210;207|207;208;216|
