# Ordered distributions and timing adjudication

B=implicitframe1; C=familyframe1; S=pinned comparable stock. Times are ms.
Only complete clean blocks support timing claims. The original complete low-memory fresh block is displayed but unranked for contention. All raw attempts remain in the archive. Scientific discrepancies are also unranked.

## development

|Screen/case; design; N/K/T; mode|B|C|S|C/S range|
|---|---|---|---|---|
|pilot/0; strong_d6; 983040/6/8; fast|1003;1011;1003|942;934;938|901;875;859|1.0455–1.0920|
|pilot/1; strong_d6; 983040/6/8; low|1187;1181;1148|1060;1078;1083|897;895;878|1.1817–1.2335|
|pilot/2; strong_d6; 491520/6/8; fast|513;517;514|479;477;481|507;502;502|0.9448–0.9582|
|pilot/3; three_fe_dense; 153600/19/8; fast|602;595;589|594;580;590|700;705;695|0.8227–0.8489|
|pilot/4; three_fe_dense; 61440/10/8; fast|161;159;160|154;160;157|230;233;232|0.6696–0.6867|
|pilot/5; strong_d6; 30720/6/4; low|75;75;74|74;73;75|119;120;120|0.6083–0.6250|
|pilot/6; strong_d6; 245760/6/1; fast|562;553;559|556;555;552|940;941;937|0.5891–0.5915|
|pilot/7; three_fe_dense; 30720/6/4; fast|76;74;76|74;74;73|78;82;82|0.8902–0.9487|
|pilot/8; one_fe; 61440/6/4; fast|66;68;65|63;62;62|41;40;41|Unranked: scientific discrepancy|

B/C paired outcomes: {'wins': 22, 'losses': 4, 'ties': 1}. Families with every comparable pair <=0.90: 4.

## fresh

|Screen/case; design; N/K/T; mode|B|C|S|C/S range|
|---|---|---|---|---|
|fresh/0; strong_d6; 737280/6/8; fast|759;765;762;765;769|712;712;718;713;723|769;762;775;767;778|0.9259–0.9344|
|fresh/1; strong_d6; 737280/6/8; low|837;841;842;847;843|791;798;790;794;809|761;771;763;777;762|Unranked: timing contention|
|fresh/2; strong_d6; 368640/6/4; fast|509;515;510;510;509|484;487;491;490;487|651;653;668;661;653|0.7350–0.7458|
|fresh/3; three_fe_dense; 184320/19/8; fast|753;748;753;756;758|738;736;732;750;737|3295;3259;3312;3266;3280|0.2210–0.2296|
|fresh/4; three_fe_dense; 76800/10/8; fast|202;198;202;207;205|200;200;207;207;199|300;299;297;297;299|0.6656–0.6970|
|fresh/5; three_fe_dense; 38400/6/8; low|122;122;121;123;125|123;126;122;124;122|118;110;110;111;126|0.9683–1.1455|
|fresh/6; strong_d6; 245760/6/1; fast|616;576;573;573;574|595;577;573;571;572|960;957;931;964;954|0.5923–0.6198|
|fresh-weak/0; weak_d3; 61440/6/4; fast|169;168;171;170;171|168;164;166;166;164|769;769;773;770;773|0.2122–0.2185|

B/C paired outcomes: {'wins': 26, 'losses': 7, 'ties': 2}. Families with every comparable pair <=0.90: 5.

## Protected OLS

|Case|B|C|
|---|---|---|
|0|65;67;67|67;69;70|
|1|81;80;80|80;83;82|
|2|10;13;10|9;9;11|
|3|223;219;222|220;249;244|
|Factor control repeat|210;212;208;209;211|208;207;210;213;216|
