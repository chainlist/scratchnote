import { m } from '#lib/paraglide/messages.js';

/** The names of the labels the classifier gives: a part of life, and for work a job family. */
const NAMES: Record<string, () => string> = {
	work: m.labels_name_work,
	health: m.labels_name_health,
	family: m.labels_name_family,
	home: m.labels_name_home,
	food: m.labels_name_food,
	money: m.labels_name_money,
	paperwork: m.labels_name_paperwork,
	transport: m.labels_name_transport,
	pets: m.labels_name_pets,
	social: m.labels_name_social,
	leisure: m.labels_name_leisure,
	travel: m.labels_name_travel,
	learning: m.labels_name_learning,
	shopping: m.labels_name_shopping,
	mind: m.labels_name_mind,
	errands: m.labels_name_errands,
	software: m.labels_name_software,
	infrastructure: m.labels_name_infrastructure,
	data: m.labels_name_data,
	sales: m.labels_name_sales,
	marketing: m.labels_name_marketing,
	support: m.labels_name_support,
	'finance-accounting': m.labels_name_finance_accounting,
	legal: m.labels_name_legal,
	management: m.labels_name_management,
	healthcare: m.labels_name_healthcare,
	education: m.labels_name_education,
	research: m.labels_name_research,
	trades: m.labels_name_trades,
	hospitality: m.labels_name_hospitality,
	retail: m.labels_name_retail,
	logistics: m.labels_name_logistics,
	manufacturing: m.labels_name_manufacturing,
	agriculture: m.labels_name_agriculture,
	creative: m.labels_name_creative,
	media: m.labels_name_media,
	'public-service': m.labels_name_public_service,
	security: m.labels_name_security,
	beauty: m.labels_name_beauty
};

/** A label in the interface language; one the app has no name for reads as it is. */
export const named = (label: string) => NAMES[label]?.() ?? label;
