export namespace backend {
	
	export enum BranchSyncStatus {
	    CREATED = 0,
	    UPDATED = 1,
	    UNCHANGED = 2,
	}
	export class CommitDetail {
	    hash: string;
	    message: string;
	    isNew: boolean;
	
	    static createFrom(source: any = {}) {
	        return new CommitDetail(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.hash = source["hash"];
	        this.message = source["message"];
	        this.isNew = source["isNew"];
	    }
	}
	export class BranchResult {
	    name: string;
	    syncStatus?: BranchSyncStatus;
	    commitCount: number;
	    commitDetails: CommitDetail[];
	    error?: string;
	
	    static createFrom(source: any = {}) {
	        return new BranchResult(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.name = source["name"];
	        this.syncStatus = source["syncStatus"];
	        this.commitCount = source["commitCount"];
	        this.commitDetails = this.convertValues(source["commitDetails"], CommitDetail);
	        this.error = source["error"];
	    }
	
		convertValues(a: any, classs: any, asMap: boolean = false): any {
		    if (!a) {
		        return a;
		    }
		    if (a.slice && a.map) {
		        return (a as any[]).map(elem => this.convertValues(elem, classs));
		    } else if ("object" === typeof a) {
		        if (asMap) {
		            for (const key of Object.keys(a)) {
		                a[key] = new classs(a[key]);
		            }
		            return a;
		        }
		        return new classs(a);
		    }
		    return a;
		}
	}
	export class ActionResult {
	    success: boolean;
	    message?: string;
	    branches: BranchResult[];
	
	    static createFrom(source: any = {}) {
	        return new ActionResult(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.success = source["success"];
	        this.message = source["message"];
	        this.branches = this.convertValues(source["branches"], BranchResult);
	    }
	
		convertValues(a: any, classs: any, asMap: boolean = false): any {
		    if (!a) {
		        return a;
		    }
		    if (a.slice && a.map) {
		        return (a as any[]).map(elem => this.convertValues(elem, classs));
		    } else if ("object" === typeof a) {
		        if (asMap) {
		            for (const key of Object.keys(a)) {
		                a[key] = new classs(a[key]);
		            }
		            return a;
		        }
		        return new classs(a);
		    }
		    return a;
		}
	}
	
	
	export class GlobalBranchPrefix {
	    branchPrefix: string;
	    error?: string;
	
	    static createFrom(source: any = {}) {
	        return new GlobalBranchPrefix(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.branchPrefix = source["branchPrefix"];
	        this.error = source["error"];
	    }
	}
	export class VcsRequest {
	    RepositoryPath: string;
	    BranchPrefix: string;
	
	    static createFrom(source: any = {}) {
	        return new VcsRequest(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.RepositoryPath = source["RepositoryPath"];
	        this.BranchPrefix = source["BranchPrefix"];
	    }
	}

}

