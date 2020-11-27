import Basic from './basic';
import config from './config';
import { Selector, t } from 'testcafe';
  fixture `test Simple Net`
    .page `${config.baseUrl}`;
  const basic = new Basic();
  test('testSimpleNet', async t => {
    basic.login('karpovrt', '123');
    const timeStamp = ''+Math.round(+new Date()/1000);
    const red = Selector('div#workflow-canvas').find('div.state-io-condition-output[colored-to="red"]').count;
    await t
      .click('#menu')
      .click('li[id="menu"] li[resource="v-s:Create"]')
      .typeText('veda-control.fulltext.dropdown', 'Сеть')
      .click('.suggestion[resource="v-wf:Net"]')
      .click('div#schema')
      .click('div#object-container div#props-col div#props table#taskTemplateProperties tbody tr td span[about="rdfs:label"]')
      .typeText('div#object-container div#props-col div#props table#taskTemplateProperties tbody tr td veda-control#VClabel input.form-control', timeStamp)
      .dragToElement('.state-io-condition-input .ep', '.glyphicon-stop')
      .click('button#workflow-save-button')
      .click('#menu')
      .click('li[id="menu"] li[resource="v-s:Create"]')
      .click('veda-control.fulltext.dropdown')
      .pressKey('ctrl+a delete')
      .typeText('veda-control.fulltext.dropdown', 'Стартовая форма')
      .click('.suggestion[resource="v-wf:StartForm"]')
      .typeText('veda-control[rel="v-wf:forNet"]', timeStamp)
      .click(Selector('.suggestions .suggestion').withText(timeStamp))
      .typeText('veda-control[rel="v-wf:hasStatusWorkflow"]', 'Ожидает отправки')
      .wait(2000)
      .click('.suggestion[resource="v-wf:ToBeSent"]')
      .click('button#save')
      .wait(2000)
      .click('button.btn.btn-link.view.edit.-search.toggle-actions')
      .click('div[rel="v-wf:isProcess"] span#label')
      .expect(red).eql(1)
});
