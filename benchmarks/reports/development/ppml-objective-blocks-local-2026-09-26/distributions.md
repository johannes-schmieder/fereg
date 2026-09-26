# Ordered distributions and timing adjudication

B=residualrows2; C=objectiveblocks1; S=pinned comparable stock. Times are ms.
These complete pilot/fresh blocks have clean monitors. Scientific discrepancies are unranked.

## development

|Screen/case; design; N/K/T; mode|B|C|S|C/S range|
|---|---|---|---|---|
|pilot/0; strong_d6; 983040/6/8; low|938;953;934|934;940;924|861;881;865|1.0670–1.0848|
|pilot/1; strong_d6; 983040/6/8; fast|864;866;862|846;845;888|868;858;873|0.9747–1.0172|
|pilot/2; strong_d6; 30720/6/4; low|72;70;70|72;72;69|122;123;120|0.5750–0.5902|
|pilot/3; three_fe_dense; 61440/10/8; fast|150;152;149|149;152;156|230;231;232|0.6478–0.6724|
|pilot/4; three_fe_dense; 153600/19/8; fast|570;570;564|569;571;566|695;702;720|0.7861–0.8187|
|pilot/5; strong_d6; 245760/6/1; fast|550;539;547|553;550;538|955;944;951|0.5657–0.5826|

B/C paired outcomes: {'wins': 9, 'losses': 7, 'ties': 2}. Families with every comparable pair <=0.90: 4.

## fresh

|Screen/case; design; N/K/T; mode|B|C|S|C/S range|
|---|---|---|---|---|
|fresh/0; strong_d6; 737280/6/8; fast|671;672;675;678;667|678;657;660;666;679|762;793;783;782;774|Unranked: scientific discrepancy|
|fresh/1; strong_d6; 737280/6/8; low|724;755;733;718;711|715;715;713;726;715|772;789;772;767;770|Unranked: scientific discrepancy|
|fresh/2; strong_d6; 368640/6/4; fast|463;456;459;463;462|455;456;461;458;457|653;667;669;671;666|0.6826–0.6968|
|fresh/3; three_fe_dense; 184320/19/8; fast|622;620;628;625;626|630;618;628;623;621|3245;3282;3253;3259;3186|0.1883–0.1949|
|fresh/4; three_fe_dense; 76800/10/8; fast|195;192;186;195;191|193;195;193;184;187|300;303;301;299;297|0.6154–0.6436|
|fresh/5; three_fe_dense; 38400/6/8; low|113;122;119;116;127|117;120;119;118;129|117;119;115;115;114|1.0000–1.1316|
|fresh/6; strong_d6; 245760/6/1; fast|557;554;560;556;559|554;553;559;557;559|962;967;970;964;961|0.5719–0.5817|
|fresh/7; weak_d3; 61440/6/4; fast|134;136;133;134;129|134;136;132;134;134|792;790;787;789;769|0.1677–0.1743|
|fresh/8; strong_d6; 61440/6/4; low|91;92;94;96;96|93;93;93;97;94|96;99;100;99;103|0.9126–0.9798|

B/C paired outcomes: {'wins': 22, 'losses': 16, 'ties': 7}. Families with every comparable pair <=0.90: 5.

## Protected OLS

|Case|B|C|
|---|---|---|
|0|66;62;69|66;64;65|
|1|77;75;82|78;77;77|
|2|16;14;16|15;14;16|
|3|211;211;214|209;211;208|
|4|159;161;158|162;163;158|
|5|18;18;19|17;17;16|
