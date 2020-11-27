/**
 * @class veda.IndividualModel
 *
 * This class is used to manipulate individuals.
 */
veda.Module(function (veda) { "use strict";

  /**
   * @constructor
   * @param {String} uri URI of individual. If not specified, than id of individual will be generated automatically.
   * @param {boolean} cache Use cache true / false. If true or not set, then object will be return from application cache (veda.cache). If false or individual not found in application cache - than individual will be loaded from database
   * @param {boolean} init individual with class model at load. If true or not set, then individual will be initialized with class specific model upon load.
   */
  veda.IndividualModel = function (uri, cache, init) {
    // veda.IndividualModel({...})
    if (typeof uri === "object" && !uri["@"]) {
      cache = uri.cache;
      init  = uri.init;
      uri   = uri.uri;
    }

    // Define Model data
    this._ = {
      cache: typeof cache === "boolean" ? cache : cache || true,
      init: typeof init !== "undefined" ? init : true,
      isNew: typeof uri === "undefined",
      isSync: typeof uri === "object",
      isLoaded: typeof uri === "object",
      pending: {},
      uri: uri
    };

    if (typeof uri === "object") {
      this.properties = uri;
      this.original = JSON.stringify(uri);
    } else {
      this.properties = {};
    }

    if (this._.cache) {
      var cached;
      if (typeof uri === "string") {
        this.id = uri;
        cached = veda.cache.get(this.id);
      } else if (typeof uri === "object") {
        cached = veda.cache.get(this.id);
        if (cached && !cached.isLoaded()) {
          cached.properties = uri;
        }
      } else if (typeof uri === "undefined") {
        this.id = veda.Util.genUri();
      }
      if (cached) {
        return cached;
      } else {
        veda.cache.set(this, this._.cache);
      }
    }

    var self = riot.observable(this);

    this.on("rdf:type", this.init);
    this.on("beforeSave", beforeSaveHandler);

    return self;
  };

  function beforeSaveHandler() {
    var now = new Date();
    var user = veda.appointment ? veda.appointment : veda.user;

    if ( !this.hasValue("v-s:creator") ) { this.set("v-s:creator", [user]); }
    if ( !this.hasValue("v-s:created") ) { this.set("v-s:created", [now]); }

    if (veda.user.id === "cfg:Administrator") {
      return;
    } else if (
      !this.hasValue("v-s:lastEditor")
      || !this.hasValue("v-s:edited")
      || this.get("v-s:lastEditor")[0].id !== user.id
      || (now - this.get("v-s:edited")[0]) > 1000
    ) {
      this.set("v-s:edited", [now]);
      this.set("v-s:lastEditor", [user]);
    }
  }

  var proto = veda.IndividualModel.prototype;

  proto.get = function (property_uri) {
    var self = this;
    if (!self.properties[property_uri]) return [];
    return self.properties[property_uri].map( parser );
  };

  proto.set = function (property_uri, values, silently) {
    this.isSync(false);
    if ( !Array.isArray(values) ) {
      values = [values];
    }
    values = values.filter(function (i) { return i !== undefined && i !== null && i !== ""; });
    var serialized = values.map(serializer).filter(Boolean);
    var uniq = unique(serialized);
    if ( JSON.stringify(uniq) !== JSON.stringify(this.properties[property_uri] || []) ) {
      if (uniq.length) {
        this.properties[property_uri] = uniq;
      } else {
        delete this.properties[property_uri];
      }
      if ( !silently ) {
        values = this.get(property_uri);
        this.trigger("propertyModified", property_uri, values);
        this.trigger(property_uri, values);
      }
    }
    return this;
  };

  function unique (arr) {
    var n = {}, r = [];
    for(var i = 0, val; i < arr.length; i++) {
      val = arr[i].type + arr[i].data + (arr[i].lang || "");
      if (!n[val]) {
        n[val] = true;
        r.push(arr[i]);
      }
    }
    return r;
  }

  // Define properties from ontology in veda.IndividualModel.prototype
  veda.IndividualModel.defineProperty = function (property_uri) {
    Object.defineProperty(proto, property_uri, {
      get: function () {
        return this.get(property_uri);
      },
      set: function (values) {
        return this.set(property_uri, values);
      },
      configurable: false,
      enumerable: false
    });
  };

  function parser(value) {
    if (value.type === "String") {
      var string = new String(value.data);
      if (value.lang !== "NONE") { string.language = value.lang; }
      return string;
    } else if (value.type === "Uri") {
      return new veda.IndividualModel(value.data);
    } else if (value.type === "Datetime") {
      return new Date(Date.parse(value.data));
    } else if (value.type === "Decimal") {
      return parseFloat(value.data);
    } else {
      return value.data;
    }
  }

  var reg_uri = /^[a-z-0-9]+:([a-zA-Z0-9-_])*$/;
  var reg_date = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d{3})?Z$/;
  var reg_ml_string = /^(.*)@([a-z]{2})$/i;

  function serializer (value) {
    if (typeof value === "number" ) {
      return {
        type: veda.Util.isInteger(value) ? "Integer" : "Decimal",
        data: value
      };
    } else if (typeof value === "boolean") {
      return {
        type: "Boolean",
        data: value
      };
    } else if (value instanceof Date) {
      return {
        type: "Datetime",
        data: value.toISOString()
      };
    } else if (value instanceof veda.IndividualModel) {
      return {
        type: "Uri",
        data: value.id
      };
    } else if (typeof value === "string" || value instanceof String) {
      if ( reg_uri.test(value) ) {
        return {
          type: "Uri",
          data: value.valueOf()
        };
      } else if ( reg_date.test(value) ) {
        return {
          type: "Datetime",
          data: value.valueOf()
        };
      } else if ( reg_ml_string.test(value) ) {
        return {
          type: "String",
          data: value.replace(reg_ml_string, "$1"),
          lang: value.replace(reg_ml_string, "$2").toUpperCase()
        };
      } else {
        return {
          type: "String",
          data: value.valueOf(),
          lang: value.language || "NONE"
        };
      }
    }
  }

  // Special properties
  Object.defineProperty(proto, "id", {
    get: function () {
      return this.properties["@"];
    },
    set: function (value) {
      var previous = this.properties && this.properties["@"];
      this.properties["@"] = value;
      if (previous && this._.cache && veda.cache.get(previous)) {
        veda.cache.remove(previous);
        veda.cache.set(this, this._.cache);
      }
    }
  });

  Object.defineProperty(proto, "membership", {
    get: function () {
      var self = this;
      //if (this._.membership) { return Promise.resolve(this._.membership); }
      if (this.isNew()) {
        this._.membership = new veda.IndividualModel({ cache: false });
        return Promise.resolve(this._.membership);
      }
      return veda.Backend.get_membership(veda.ticket, this.id).then(function (membershipJSON) {
        self._.membership = new veda.IndividualModel({ uri: membershipJSON, cache: false });
        return self._.membership.load();
      }).catch(function  (error) {
        console.log("membership error", self.id, error);
        self._.membership = new veda.IndividualModel({ cache: false });
        return self._.membership.load();
      });
    },
    configurable: false,
    enumerable: false
  });

  proto.memberOf = function () {
    return this.membership.then(function (membership) {
      return membership.hasValue("v-s:memberOf") ? membership.properties["v-s:memberOf"].map(function (group_item) {
        return group_item.data;
      }) : [];
    })
  };

  proto.isMemberOf = function (group_uri) {
    return this.membership.then(function (membership) {
      return membership.hasValue("v-s:memberOf", group_uri);
    });
  };

  Object.defineProperty(proto, "rights", {
    get: function () {
      var self = this;
      //if (this._.rights) { return Promise.resolve(this._.rights); }
      if (this.isNew()) {
        this._.rights = new veda.IndividualModel({ cache: false });
        this._.rights["v-s:canCreate"] = [ true ];
        this._.rights["v-s:canRead"] = [ true ];
        this._.rights["v-s:canUpdate"] = [ true ];
        this._.rights["v-s:canDelete"] = [ true ];
        return Promise.resolve(this._.rights);
      }
      return veda.Backend.get_rights(veda.ticket, this.id).then(function (rightsJSON) {
        return self._.rights = new veda.IndividualModel( rightsJSON, false );
      }).catch(function  (error) {
        console.log("rights error", self.id, error);
        return self._.rights = new veda.IndividualModel({ cache: false });
      });
    },
    configurable: false,
    enumerable: false
  });

  proto.can = function (action) {
    action = action.charAt(0).toUpperCase() + action.slice(1).toLowerCase();
    return this.rights.then(function (rights) {
      return rights.hasValue("v-s:can" + action, true);
    });
  };
  proto.canCreate = function () {
    return this.can("Create");
  };
  proto.canRead = function () {
    return this.can("Read");
  };
  proto.canUpdate = function () {
    return this.can("Update");
  };
  proto.canDelete = function () {
    return this.can("Delete");
  };

  Object.defineProperty(proto, "rightsOrigin", {
    get: function () {
      var self = this;
      //if (this._.rightsOrigin) { return Promise.resolve(this._.rightsOrigin); }
      return veda.Backend.get_rights_origin(veda.ticket, this.id).then(function (rightsOriginArr) {
        return self._.rightsOrigin = Promise.all(rightsOriginArr.map(function (item) {
          return new veda.IndividualModel( item, false );
        }));
      }).catch(function  (error) {
        console.log("rights error", self.id, error);
        return self._.rightsOrigin = [];
      });
    },
    configurable: false,
    enumerable: false
  });

  /**
   * @method
   * Load individual specified by uri from database. If cache parameter (from constructor) is true, than try to load individual from browser cache first.
   * @param {String} uri individual uri
   */
  proto.load = function () {
    if ( this.isLoading() && typeof window !== "undefined" ) {
      return this.isLoading();
    }
    var self = this;
    this.trigger("beforeLoad");
    if ( this.isLoaded() && ( veda.Backend.status === "online" || veda.Backend.status === "offline" ) ) {
      this.trigger("afterLoad", this);
      return Promise.resolve( this );
    } else if ( this.isLoaded() && veda.Backend.status === "limited" ) {
      if (typeof window !== "undefined") {
        return this.is("v-s:UserThing").then(function (isUserThing) {
          if (isUserThing) {
            return self.reset();
          } else {
            return self;
          }
        }).then(function (self) {
          self.trigger("afterLoad", self);
          return self;
        });
      } else {
        return self.reset()
        .then(function (self) {
          self.trigger("afterLoad", self);
          return self;
        });
      }
    }
    var uri = this._.uri ;
    if (typeof uri === "string") {
      var loadingPromise = veda.Backend.get_individual(veda.ticket, uri).then(function (individualJson) {
        self.isLoading(false);
        self.isNew(false);
        self.isSync(true);
        self.isLoaded(true);
        self.properties = individualJson;
        self.original = JSON.stringify(individualJson);
        self.trigger("afterLoad", self);
        if (self._.init) {
          return self.init();
        }
        return self;
      }).catch(function (error) {
        self.isLoading(false);
        console.log("load individual error", self.id, error);
        if (error.code === 422 || error.code === 404) {
          self.isNew(true);
          self.isSync(false);
          self.isLoaded(false);
          self.properties = {
            "@": uri,
            "rdf:type": [{type: "Uri", data: "rdfs:Resource"}],
            "rdfs:label": [
              {type: "String", data: "Объект не существует [" + uri + "]", lang: "RU"},
              {type: "String", data: "Object does not exist [" + uri + "]", lang: "EN"}
            ]
          };
        } else if (error.code === 472) {
          self.isNew(false);
          self.isSync(false);
          self.isLoaded(false);
          self.properties = {
            "@": uri,
            "rdf:type": [{type: "Uri", data: "rdfs:Resource"}],
            "rdfs:label": [
              {type: "String", data: "Нет прав на объект", lang: "RU"},
              {type: "String", data: "Insufficient rights", lang: "EN"}
            ]
          };
        } else if (error.code === 470 || error.code === 471) {
          self.isNew(false);
          self.isSync(false);
          self.isLoaded(false);
        } else if (error.code === 0 || error.code === 4000 || error.code === 503) {
          self.isNew(false);
          self.isSync(false);
          self.isLoaded(false);
          self.properties = {
            "@": uri,
            "rdf:type": [{type: "Uri", data: "rdfs:Resource"}],
            "rdfs:label": [
              {type: "String", data: "Нет связи с сервером. Этот объект сейчас недоступен.", lang: "RU"},
              {type: "String", data: "Server disconnected. This object is not available now.", lang: "EN"}
            ]
          };
          veda.one("online", function () {
            self.reset();
          });
        } else {
          self.isNew(false);
          self.isSync(false);
          self.isLoaded(false);
          self.properties = {
            "@": uri,
            "rdf:type": [{type: "Uri", data: "rdfs:Resource"}],
            "rdfs:label": [{type: "String", data: uri, lang: "NONE"}]
          };
        }
        self.trigger("afterLoad", self);
        return self;
      });
      return this.isLoading(loadingPromise);

    } else if (typeof uri === "object") {
      this.isNew(false);
      this.isSync(true);
      this.isLoaded(true);
      this.properties = uri;
    } else if (typeof uri === "undefined") {
      this.isNew(true);
      this.isSync(false);
      this.isLoaded(false);
    }
    this.trigger("afterLoad", this);
    if (this._.init) {
      return this.init();
    }
    return Promise.resolve(this);
  };

  /**
   * @method
   * Save current individual to database
   */
  proto.save = function() {
    // Do not save individual to server if nothing changed
    if (this.isSync()) { return Promise.resolve(this); }
    if ( this.isSaving() && this.isSync() && typeof window !== "undefined" ) {
      return this.isSaving();
    }
    // Do not save rdfs:Resource
    if ( this.hasValue("rdf:type", "rdfs:Resource") ) {
      var notify = veda.Notify ? new veda.Notify() : console.log;
      notify("danger", { message: "Не могу сохранить объект типа rdfs:Resource" });
      return this;
    }
    var self = this;
    this.trigger("beforeSave");
    Object.keys(this.properties).reduce(function (acc, property_uri) {
      if (property_uri === "@") return acc;
      acc[property_uri] = acc[property_uri].filter(function (item) {
        return item && item.data !== "" && item.data !== undefined && item.data !== null;
      });
      if (!acc[property_uri].length) delete acc[property_uri];
      return acc;
    }, this.properties);

    var original = this.original ? JSON.parse(this.original, veda.Util.decimalDatetimeReviver) : {"@": this.id};
    var delta = veda.Util.diff(this.properties, original);

    var promise = (this.isNew() ?
      veda.Backend.put_individual(veda.ticket, this.properties) :
      Promise.all([
        delta.added && Object.keys(delta.added).length ? (delta.added["@"] = this.id, veda.Backend.add_to_individual(veda.ticket, delta.added)) : undefined,
        delta.differ && Object.keys(delta.differ).length ? (delta.differ["@"] = this.id, veda.Backend.set_in_individual(veda.ticket, delta.differ)) : undefined,
        delta.missing && Object.keys(delta.missing).length? (delta.missing["@"] = this.id, veda.Backend.remove_from_individual(veda.ticket, delta.missing)) : undefined
      ])
    ).then(function () {
      self.original = JSON.stringify(self.properties);
      self.isSaving(false);
      self.isNew(false);
      self.isSync(true);
      self.isLoaded(true);
      self.trigger("afterSave");
      return self;
    }).catch(function (error) {
      self.isSaving(false);
      console.log("save individual error", self.id, error);
      throw error;
    });

    return this.isSaving(promise);
  }

  /**
   * @method
   * Reset current individual to  database
   */
  proto.reset = function (original) {
    var self = this;
    if ( this.isResetting() && typeof window !== "undefined" ) {
      return this.isResetting();
    }
    this.trigger("beforeReset");
    if (this.isNew()) {
      this.trigger("afterReset");
      return Promise.resolve(this);
    }
    var promise = (original ? Promise.resove(original) : veda.Backend.reset_individual(veda.ticket, self.id))
      .then(processOriginal)
      .then(function () {
        self.isResetting(false);
        self.isNew(false);
        self.isSync(true);
        self.isLoaded(true);
        self.trigger("afterReset");
        return self;
      })
      .catch(function (error) {
        self.isResetting(false);
        console.log("reset individual error", self.id, error);
        throw error;
      });
    return self.isResetting(promise);

    function processOriginal(original) {
      self.original = JSON.stringify(original);
      var self_property_uris = Object.keys(self.properties);
      var original_property_uris = Object.keys(original);
      var union = veda.Util.unique( self_property_uris.concat(original_property_uris) );
      union.forEach(function (property_uri) {
        var modified = false;
        if (property_uri === "@") { return; }
        if (!self.properties[property_uri]) {
          self.properties[property_uri] = original[property_uri];
          modified = true;
        } else if (!original[property_uri]) {
          delete self.properties[property_uri];
          modified = true;
        } else {
          var currentSum = JSON.stringify(self.properties[property_uri]).split("").reduce(function (acc, char) {return acc += char.charCodeAt(0);}, 0);
          var originalSum = JSON.stringify(original[property_uri]).split("").reduce(function (acc, char) {return acc += char.charCodeAt(0);}, 0);
          if (currentSum !== originalSum) {
            self.properties[property_uri] = original[property_uri];
            modified = true;
          }
        }
        if (modified) {
          var values = self.get(property_uri);
          self.trigger("propertyModified", property_uri, values);
          self.trigger(property_uri, values);
        }
      });
    }
  };

  /**
   * @method
   * Mark current individual as deleted in database (add v-s:deleted property)
   */
  proto.delete = function () {
    this.trigger("beforeDelete");
    if ( this.isNew() ) {
      this.trigger("afterDelete");
      return Promise.resolve(this);
    }
    this["v-s:deleted"] = [ true ];
    this.trigger("afterDelete");
    return this.save();
  };

  /**
   * @method
   * Remove individual from database
   */
  proto.remove = function () {
    var self = this;
    this.trigger("beforeRemove");
    if ( this._.cache && veda.cache && veda.cache.get(this.id) ) {
      veda.cache.remove(this.id);
    }
    if ( this.isNew() ) {
      this.trigger("afterRemove");
      return Promise.resolve(this);
    }
    return veda.Backend.remove_individual(veda.ticket, this.id).then(function () {
      self.trigger("afterRemove");
      return self;
    });
  };

  /**
   * @method
   * Recover current individual in database (remove v-s:deleted property)
   */
  proto.recover = function () {
    this.trigger("beforeRecover");
    this["v-s:deleted"] = [];
    this.trigger("afterRecover");
    return this.save();
  };

  /**
   * @method
   * @param {String} property_uri property name
   * @return {boolean} is requested property exists in this individual
   */
  proto.hasValue = function (property_uri, value) {
    if (!property_uri && typeof value !== "undefined" && value !== null) {
      var found = false;
      for (var property_uri in this.properties) {
        if (property_uri === "@") { continue; }
        found = found || this.hasValue(property_uri, value);
      }
      return found;
    }
    var result = !!(this.properties[property_uri] && this.properties[property_uri].length);
    if (typeof value !== "undefined" && value !== null) {
      var serialized = serializer(value);
      result = result && !!this.properties[property_uri].filter( function (item) {
        return ( item.data == serialized.data && (item.lang && serialized.lang ? item.lang === serialized.lang : true) );
      }).length;
    }
    return result;
  };

  /**
   * @method
   * @param {String} property_uri property name
   * @param {Any allowed type} value
   * @return {this}
   */
  proto.addValue = function (property_uri, values, silently) {
    if (typeof values === "undefined" || values === null) {
      return this;
    }
    this.properties[property_uri] = this.properties[property_uri] || [];
    if ( Array.isArray(values) ) {
      var that = this;
      values.forEach(function (value) {
        addSingleValue.call(that, property_uri, value);
      });
    } else {
      addSingleValue.call(this, property_uri, values);
    }
    this.isSync(false);
    if ( !silently ) {
      values = this.get(property_uri);
      this.trigger("propertyModified", property_uri, values);
      this.trigger(property_uri, values);
    }
    return this;
  };
  function addSingleValue(property_uri, value) {
    if (value != undefined) {
      var serialized = serializer(value);
      this.properties[property_uri].push(serialized);
    }
  }

  /**
   * @method
   * @param {String} property_uri property name
   * @param {Any allowed type} value
   * @return {this}
   */
  proto.removeValue = function (property_uri, values, silently) {
    if (!this.properties[property_uri] || !this.properties[property_uri].length || typeof values === "undefined" || values === null) {
      return this;
    }
    if ( Array.isArray(values) ) {
      var that = this;
      values.forEach(function (value) {
        removeSingleValue.call(that, property_uri, value);
      });
    } else {
      removeSingleValue.call(this, property_uri, values);
    }
    this.isSync(false);
    if ( !silently ) {
      values = this.get(property_uri);
      this.trigger("propertyModified", property_uri, values);
      this.trigger(property_uri, values);
    }
    return this;
  };
  function removeSingleValue (property_uri, value) {
    if (value != undefined) {
      var serialized = serializer(value);
      this.properties[property_uri] = (this.properties[property_uri] || []).filter(function (item) {
        return !( item.data == serialized.data && (item.lang && serialized.lang ? item.lang === serialized.lang : true) );
      });
    }
  }

  /**
   * @method
   * @param {String} property_uri property name
   * @param {Any allowed type} value
   * @return {this}
   */
  proto.toggleValue = function (property_uri, values, silently) {
    if (typeof values === "undefined" || values === null) {
      return this;
    }
    this.properties[property_uri] = this.properties[property_uri] || [];
    if ( Array.isArray(values) ) {
      var that = this;
      values.forEach(function (value) {
        toggleSingleValue.call(that, property_uri, value);
      });
    } else {
      toggleSingleValue.call(this, property_uri, values);
    }
    this.isSync(false);
    if ( !silently ) {
      values = this.get(property_uri);
      this.trigger("propertyModified", property_uri, values);
      this.trigger(property_uri, values);
    }
    return this;
  };
  function toggleSingleValue (property_uri, value) {
    if (value != undefined) {
      if ( this.hasValue(property_uri, value) ) {
        removeSingleValue.call(this, property_uri, value);
      } else {
        addSingleValue.call(this, property_uri, value);
      }
    }
  }

  /**
   * @method
   * @param {String} property_uri property name
   * @return {this}
   */
  proto.clearValue = function (property_uri, silently) {
    if (!this.properties[property_uri] || !this.properties[property_uri].length) {
      return this;
    } else {
      delete this.properties[property_uri];
      this.isSync(false);
      if ( !silently ) {
        var empty = [];
        this.trigger("propertyModified", property_uri, empty);
        this.trigger(property_uri, empty);
      }
    }
    return this;
  };

  /**
   * @method
   * @param {String} id of class to check
   * @return {boolean} is individual rdf:type subclass of requested class
   */
  proto.is = function (_class) {
    var self = this;
    if (typeof _class.valueOf() === "string") {
      _class = new veda.IndividualModel( _class.valueOf() );
    }
    var types = self.get("rdf:type");
    var is = eval(
      types.map(function (type) {
        return self.hasValue("rdf:type", _class.id);
      }).join("||")
    );
    if (is) {
      return Promise.resolve(is);
    } else {
      return Promise.all(types.map(isSub)).then(function (results) {
        return eval(results.join("||"));
      });
    }

    function isSub(type) {
      if (is) { return is; }
      if (!type.hasValue("rdfs:subClassOf")) {
        return (is = is || false);
      } else if (type.hasValue("rdfs:subClassOf", _class.id)) {
        return (is = is || true);
      } else {
        var types = type.get("rdfs:subClassOf");
        return Promise.all(types.map(isSub)).then(function (results) {
          return eval(results.join("||"));
        });
      }
    }
  };

  /**
   * @method
   * Initialize individual with class specific domain properties and methods
   */
  proto.init = function () {
    var self = this;
    var isClass = this.hasValue("rdf:type", "owl:Class") || this.hasValue("rdf:type", "rdfs:Class");
    if ( this.hasValue("v-ui:hasModel") && !isClass ) {
      return this.get("v-ui:hasModel")[0].load()
        .then(function (model) {
          if ( !model.modelFn ) {
            model.modelFn = new Function(model["v-s:script"][0]);
          }
          model.modelFn.call(self);
          return self;
        });
    } else {
      var types_promises = this.get("rdf:type").map( function (type_promise) {
        return type_promise.load();
      });
      return Promise.all( types_promises )
        .then( function (types) {
          var models_promises = [];
          types.map( function (type) {
            if ( type.hasValue("v-ui:hasModel") ) {
              models_promises.push( type.get("v-ui:hasModel")[0].load() );
            }
          });
          return Promise.all( models_promises );
        })
        .then( function (models) {
          models.map(function (model) {
            if ( !model.modelFn ) {
              model.modelFn = new Function(model.get("v-s:script")[0]);
            }
            model.modelFn.call(self);
          });
          return self;
        });
    }
  };

  /**
   * @method
   * Clone individual with different (generated) id
   * @return {veda.IndividualModel} clone of this individual with different id.
   */
  proto.clone = function () {
    var cloneProperties = JSON.parse( JSON.stringify(this.properties), veda.Util.decimalDatetimeReviver );
    cloneProperties["@"] = veda.Util.genUri();
    var clone = new veda.IndividualModel(cloneProperties);
    clone.isNew(true);
    clone.isSync(false);
    clone.clearValue("v-s:updateCounter");
    return clone.init();
  };

  /**
   * @method
   * Check whether individual is synchronized with db
   * @return {boolean}
   */
  proto.isSync = function (value) {
    return ( typeof value !== "undefined" ? this._.isSync = value : this._.isSync );
  };

  /**
   * @method
   * Check whether individual is new (not saved in db)
   * @return {boolean}
   */
  proto.isNew = function (value) {
    return ( typeof value !== "undefined" ? this._.isNew = value : this._.isNew );
  };

  /**
   * @method
   * Check whether individual was loaded from db
   * @return {boolean}
   */
  proto.isLoaded = function (value) {
    return ( typeof value !== "undefined" ? this._.isLoaded = value : this._.isLoaded );
  };

  proto.isPending = function(operation, value) {
    return ( typeof value !== "undefined" ? this._.pending[operation] = value : this._.pending[operation] );
  }

  proto.isLoading = function (value) {
    return this.isPending("load", value);
  };
  proto.isSaving = function (value) {
    return this.isPending("save", value);
  };
  proto.isResetting = function (value) {
    return this.isPending("reset", value);
  };

  /**
   * @method
   * Serialize to JSON
   * @return {Object} JSON representation of individual.
   */
  proto.toJson = function () {
    return this.properties;
  };

  /**
   * @method
   * Serialize to string
   * @return {String} String representation of individual.
   */
  proto.toString = function () {
    return this.hasValue("rdfs:label") ? this.get("rdfs:label").map(veda.Util.formatValue).join(" ") : this.hasValue("rdf:type") ? this.get("rdf:type")[0].toString() + ": " + this.id : this.id ;
  };

  /**
   * @method
   * Return self
   * @return {Object} self.
   */
  proto.valueOf = function () {
    return this.id;
  };

  /**
   * @method
   * Get values for first property chain branch.
   * @param {property_uri, ...} Property chain to get values.
   */
  proto.getPropertyChain = function () {
    var args = Array.prototype.slice.call(arguments);
    var property_uri = args.shift();
    return this.load().then(function (self) {
      if ( self.hasValue(property_uri) ) {
        if ( !args.length ) {
          return self[property_uri];
        } else {
          return self.getPropertyChain.apply(self[property_uri][0], args);
        }
      }
      return [];
    }).catch(function (error) {
      console.log(error);
    });
  };

  /**
   * @method
   * Get values for all property chain branches.
   * @param {property_uri, ...} Property chain to get values.
   */
  proto.getChainValue = function () {
    var individuals = this;
    if ( !Array.isArray(individuals) ) {
      individuals = [individuals];
    }
    var properties = Array.prototype.slice.call(arguments);
    var property_uri = properties.shift();
    var promises = individuals.map(function (individual) {
      return individual.load();
    });
    return Promise.all(promises).then(function (individuals) {
      var children = individuals.reduce(function (acc, individual) {
        return acc.concat(individual.get(property_uri));
      }, []);
      if ( !properties.length ) {
        return children;
      } else {
        return proto.getChainValue.apply(children, properties);
      }
    }).catch(function (error) {
      console.log(error);
      return [];
    });
  };

  /**
   * @method
   * Check value for all property chain branches.
   * @param {property_uri, ..., value} Property chain and a value to check.
   */
  proto.hasChainValue = function () {
    var length = arguments.length;
    var sought_value = arguments[length - 1];
    var args = Array.prototype.slice.call(arguments, 0, length - 1);
    return this.getChainValue.apply(this, args).then(function (values) {
      return values.reduce(function (state, value) {
        return state || sought_value.valueOf() == value.valueOf();
      }, false);
    });
  };

  /**
   * @method
   * Prefetch linked objects. Useful for presenting objects with many links.
   * @param {Number} Depth of the object tree to prefetch.
   * @param {allowed_property_uri, ...} Allowed property uri for links. If defined the tree is formed only for allowed properties.
   */
  proto.prefetch = function (depth) {
    var allowed_props = [].slice.call(arguments, 1);
    depth = depth || 1;
    return this.load().then(function (self) {
      return prefetch.apply(self, [[], depth, [self.id]].concat(allowed_props) );
    });
  };

  function prefetch(result, depth, uris) {
    var self = this;
    var allowed_props = [].slice.call(arguments, 3);
    uris = veda.Util.unique( uris );
    var toGet = uris.filter(function (uri) {
      var cached = veda.cache.get(uri);
      if ( cached && result.indexOf(cached) < 0 ) {
        result.push(cached);
      }
      return !cached;
    });
    return (toGet.length ? veda.Backend.get_individuals(veda.ticket, toGet) : Promise.resolve([])).then(function (got) {
      var nextUris = [];
      got.forEach(function (json) {
        if (json) {
          var individual = new veda.IndividualModel(json);
          if ( result.indexOf(individual) < 0 ) {
            result.push(individual);
          }
        }
      });
      if (depth - 1 === 0) { return result; }
      uris.forEach(function (uri) {
        var individual = new veda.IndividualModel(uri);
        var data = individual.properties;
        Object.keys(data).forEach( function (key) {
          if ( key === "@" || (allowed_props.length && allowed_props.indexOf(key) < 0) ) { return; }
          data[key].map(function (value) {
            if (value.type === "Uri") {
              nextUris.push(value.data);
            }
          });
        });
      });
      if (!nextUris.length) { return result; }
      return prefetch.apply(self, [result, depth-1, nextUris].concat(allowed_props) );
    });
  }

});
